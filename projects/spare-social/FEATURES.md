# Spare Social: everything to build

*Working name. Stage: Mapping. Generated from [features.json](features.json); edit that file, not this one. The architecture is in [ARCHITECTURE.md](ARCHITECTURE.md) and the work board in [STATUS.md](STATUS.md).*

Spare Social replaces TikTok, YouTube, Instagram and Facebook with one app that is free to join. It runs at cost with no adverts, feeds people choose with published rules, removal of only what the law requires, and open protocols so members can take their followers anywhere. This map lists everything a full replacement needs, top down: 23 areas, 199 features a person can use, and 525 buildable parts. 108 features are core, meaning a first usable release needs them; the rest are next (soon after) or later. Each part names what it depends on: Spare Kit crates (kit-), kit services (svc-), shared engines (engine-), parts of Spare Kit by their KIT- ID (see the [kit map](../kit/FEATURES.md)) or other Spare Social parts by ID. Where a part waits on a research or decision piece, the piece links to its work board.

## Areas at a glance

| Area | Features | Core | Parts |
| --- | --- | --- | --- |
| [SOC-A1 Joining, accounts and profiles](#soc-a1-joining-accounts-and-profiles) | 7 | 5 | 27 |
| [SOC-A2 Friends, follows and audiences](#soc-a2-friends-follows-and-audiences) | 8 | 3 | 17 |
| [SOC-A3 Posts and conversation](#soc-a3-posts-and-conversation) | 11 | 7 | 35 |
| [SOC-A4 Photos and stories](#soc-a4-photos-and-stories) | 8 | 3 | 16 |
| [SOC-A5 Short videos](#soc-a5-short-videos) | 7 | 4 | 22 |
| [SOC-A6 Long videos and channels](#soc-a6-long-videos-and-channels) | 10 | 5 | 31 |
| [SOC-A7 Live streaming](#soc-a7-live-streaming) | 9 | 0 | 17 |
| [SOC-A8 Groups, pages and events](#soc-a8-groups-pages-and-events) | 8 | 3 | 29 |
| [SOC-A9 Messages and calls](#soc-a9-messages-and-calls) | 12 | 6 | 29 |
| [SOC-A10 Feeds, discovery and search](#soc-a10-feeds-discovery-and-search) | 7 | 6 | 22 |
| [SOC-A11 Human and AI sections](#soc-a11-human-and-ai-sections) | 7 | 5 | 17 |
| [SOC-A12 Personal controls and filters](#soc-a12-personal-controls-and-filters) | 6 | 5 | 15 |
| [SOC-A13 Moderation, legal duties and transparency](#soc-a13-moderation-legal-duties-and-transparency) | 14 | 12 | 49 |
| [SOC-A14 Young people and wellbeing](#soc-a14-young-people-and-wellbeing) | 10 | 5 | 24 |
| [SOC-A15 Creator support and money](#soc-a15-creator-support-and-money) | 13 | 2 | 27 |
| [SOC-A16 Creator studio and stats](#soc-a16-creator-studio-and-stats) | 6 | 1 | 10 |
| [SOC-A17 Media pipeline and delivery](#soc-a17-media-pipeline-and-delivery) | 8 | 7 | 27 |
| [SOC-A18 Open network and moving between networks](#soc-a18-open-network-and-moving-between-networks) | 6 | 4 | 16 |
| [SOC-A19 Your data and privacy](#soc-a19-your-data-and-privacy) | 8 | 6 | 20 |
| [SOC-A20 Apps, devices, offline and notifications](#soc-a20-apps-devices-offline-and-notifications) | 9 | 5 | 21 |
| [SOC-A21 Accessibility and languages](#soc-a21-accessibility-and-languages) | 9 | 6 | 16 |
| [SOC-A22 Running the co-op: rules, costs, admin and support](#soc-a22-running-the-co-op-rules-costs-admin-and-support) | 8 | 6 | 23 |
| [SOC-A23 Headless API, plugins and AI agents](#soc-a23-headless-api-plugins-and-ai-agents) | 8 | 2 | 15 |

Kinds of part: **library** (code compiled into the app), **service** (runs on servers), **interface** (screens), **data** (records and formats), **integration** (joins a kit service or outside provider), **research** and **legal** (notes and checks before building). Sizes follow the work board: small, medium, large.

## SOC-A1 Joining, accounts and profiles

Signing up with one Spareware login, making a profile, proving who you are when it matters, running several profiles, finding people you know, getting back in, and leaving cleanly.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A1-F1** Sign up with one Spareware login | Join in under a minute with a passkey, a handle and a display name, using the same login as every other Spareware app. | core | P1 Spareware login hook-up; P2 Sign-up flow; P3 Handle rules; P4 Sign-up spam guard |
| **SOC-A1-F2** Make a profile | Add a photo, banner, bio, pronouns, links and an optional place, and see your videos, photos and posts on tabs. | core | P1 Profile record; P2 Profile page; P3 Profile links list; P4 Pinned and featured posts |
| **SOC-A1-F3** Prove who you are when it matters | Get an optional badge that shows a real person, organisation or public figure is who they say, or use your own web address as your handle. | next | P1 Own web address as handle; P2 Optional identity check; P3 Organisation check; P4 Badge rules |
| **SOC-A1-F4** Run several profiles from one login | Keep a personal profile, a creator channel and a page, and switch between them in one tap. | next | P1 Several profiles per login; P2 Profile switcher; P3 Shared roles on a profile; P4 Switch logins on a shared device |
| **SOC-A1-F5** Find people you know | Invite friends by link or QR code, find contacts who agree to be found, and pick some interests to start. | core | P1 Invite links and QR codes; P2 Contact matching with consent; P3 Find friends from other networks; P4 Starting interests |
| **SOC-A1-F6** Get back into your account | Recover a lost login, fix a hacked account and sign out of devices you do not recognise. | core | P1 Recovery options; P2 Hacked account repair; P3 Sessions list; P4 Sign-in alerts |
| **SOC-A1-F7** Pause, leave or hand on your account | Hide your account for a while, delete it for good, or name someone to look after it when you die. | core | P1 Pause and delete; P2 Memorial accounts and legacy contact; P3 Deceased members' accounts |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A1-F1-P1 | **Spareware login hook-up**: Use the kit's account service and passkey sign-in (KIT-A1-F2-P1), passwords as a fallback. | integration | small | KIT-A1-F1-P1, KIT-A1-F2-P1 |  |
| SOC-A1-F1-P2 | **Sign-up flow**: Screens for handle, display name and agreeing to the published rules, plus a date of birth checked against the age rules; only the age band and the month it next changes are kept, never the full date, matching the kit's promise to prove age without sharing more (KIT-A2-F8). | interface | medium | SOC-A1-F1-P1, SOC-A14-F1-P1, SOC-A13-F2-P1, KIT-A1-F1-P5 |  |
| SOC-A1-F1-P3 | **Handle rules**: Unique handles, reserved words, look-alike and impersonation checks, a history of past handles, and a waiting period before a released handle can be taken again. | service | small | kit-core |  |
| SOC-A1-F1-P4 | **Sign-up spam guard**: Uses the kit's sign-up abuse protection (KIT-A1-F1-P4), where the proof-of-work check belongs, with limits set by Spare Social's rules. | integration | small | KIT-A1-F1-P4 |  |
| SOC-A1-F2-P1 | **Profile record**: Name, handle, photo, banner, bio, pronouns, links, place and chosen tabs, stored as operations. | data | small | kit-core, kit-ops |  |
| SOC-A1-F2-P2 | **Profile page**: One page with tabs for short videos, long videos, photos, posts, groups and events. | interface | medium | SOC-A1-F2-P1 |  |
| SOC-A1-F2-P3 | **Profile links list**: Several links on the profile, opened directly with no redirect tracking. | data | small | SOC-A1-F2-P1 |  |
| SOC-A1-F2-P4 | **Pinned and featured posts**: Pin up to a few posts or videos to the top of the profile. | data | small | SOC-A3-F1-P1 |  |
| SOC-A1-F3-P1 | **Own web address as handle**: Prove an account owns a domain name by a DNS record or a file on the site. | service | small | SOC-A18-F1-P2 | [M01](STATUS.md) |
| SOC-A1-F3-P2 | **Optional identity check**: Uses the kit's identity check (KIT-A2-F1-P2) through an accredited provider, keeping only the result. | integration | small | KIT-A2-F1-P2 |  |
| SOC-A1-F3-P3 | **Organisation check**: Uses the kit's business number and charities register checks (KIT-A2-F7-P1, KIT-A2-F7-P3). | integration | small | KIT-A2-F7-P1, KIT-A2-F7-P3 |  |
| SOC-A1-F3-P4 | **Badge rules**: Published rules for what each badge means and how one is lost, shown on every badge. | interface | small | SOC-A13-F2-P1, KIT-A2-F5-P1 |  |
| SOC-A1-F4-P1 | **Several profiles per login**: One login owns several profiles, each with its own followers and settings. | data | medium | KIT-A1-F5-P1, KIT-A1-F1-P1, kit-core |  |
| SOC-A1-F4-P2 | **Profile switcher**: A quick switcher that always shows which profile you are acting as. | interface | small | SOC-A1-F4-P1 |  |
| SOC-A1-F4-P3 | **Shared roles on a profile**: Give other people owner, editor, moderator or viewer roles on a profile, using the kit's team roles (KIT-A1-F7-P1) and the group roles library (SOC-A8-F2-P1). | service | small | SOC-A1-F4-P1, SOC-A8-F2-P1, KIT-A1-F7-P1 |  |
| SOC-A1-F4-P4 | **Switch logins on a shared device**: Keep several people's logins on one shared phone or computer and switch between them, each locked by its own passkey. | interface | small | KIT-A1-F2-P1, KIT-A1-F3-P2 |  |
| SOC-A1-F5-P1 | **Invite links and QR codes**: Personal invite links and QR codes that follow back automatically if both agree. | service | small | SOC-A2-F1-P1 |  |
| SOC-A1-F5-P2 | **Contact matching with consent**: Match phone contacts with a proven private contact discovery method, not plain hashes that are easy to reverse, only against people who allow it, never keeping the list; a candidate kit part, since other apps need it too, with notices under privacy principle 3A where anything is kept (SOC-A19-F4-P4). | service | medium | KIT-A1-F1-P1, SOC-A19-F4-P4 |  |
| SOC-A1-F5-P3 | **Find friends from other networks**: Look up people you follow elsewhere through the open protocols. | integration | medium | SOC-A18-F1-P2 | [M01](STATUS.md) |
| SOC-A1-F5-P4 | **Starting interests**: Pick topics and feeds to start with; the choice only seeds feeds you choose, nothing is tracked. | interface | small | SOC-A10-F2-P3 |  |
| SOC-A1-F6-P1 | **Recovery options**: Recovery codes, a backup passkey and assisted recovery from the kit (KIT-A1-F4). | integration | small | KIT-A1-F4-P1, KIT-A1-F4-P2 |  |
| SOC-A1-F6-P2 | **Hacked account repair**: Lock the account, review what changed and undo the attacker's actions from the operation log. | service | medium | kit-ops, KIT-A1-F3-P2 |  |
| SOC-A1-F6-P3 | **Sessions list**: See every signed-in device and sign out of any or all of them. | interface | small | KIT-A1-F3-P2 |  |
| SOC-A1-F6-P4 | **Sign-in alerts**: Uses the kit's new sign-in alerts (KIT-A1-F3-P3) to tell the member straight away about a sign-in from a new device. | integration | small | KIT-A1-F3-P3 |  |
| SOC-A1-F7-P1 | **Pause and delete**: Deactivate to hide everything, or delete with a short grace period and then a full purge. | service | medium | SOC-A17-F6-P1, SOC-A19-F1-P1, KIT-A18-F2-P1 |  |
| SOC-A1-F7-P2 | **Memorial accounts and legacy contact**: Turn a profile into a memorial on request and let a named contact manage it, using the kit's after-death handling (KIT-A18-F11-P1). | integration | medium | KIT-A18-F11-P1, SOC-A1-F1-P1 |  |
| SOC-A1-F7-P3 | **Deceased members' accounts**: What happens to a deceased member's public posts, videos and memberships, on top of the kit's after-death rules (KIT-A18-F11-P1). | legal | small | KIT-A18-F11-P1 | [M02](STATUS.md) |

## SOC-A2 Friends, follows and audiences

Who you are connected to: one-way follows, two-way friends, close friends and lists, follower requests, suggestions with a visible reason, and reminders for the people who matter.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A2-F1** Follow anyone | Follow a person, channel, page or group and see their posts, with followers and following lists on each profile. | core | P1 Social graph store; P2 Follow button and lists; P3 Private accounts |
| **SOC-A2-F2** Be friends both ways | Send a friend request, accept or decline it, and share friends-only posts with people you both chose. | core | P1 Friend requests; P2 Requests inbox |
| **SOC-A2-F3** Close friends and lists | Make lists such as close friends, family or work, and post or browse just for that list. | next | P1 Custom lists; P2 List manager |
| **SOC-A2-F4** Suggestions with a visible reason | See people you may know, each with the reason shown, and turn suggestions off if you like. | next | P1 Suggestion rules; P2 Why this suggestion |
| **SOC-A2-F5** Manage your followers | Remove a follower without blocking them, hide your counts, and see who follows you. | core | P1 Remove follower and hide follower counts |
| **SOC-A2-F6** Birthdays and reminders | Share your birthday with friends if you want and get a reminder for theirs. | later | P1 Birthday sharing; P2 Reminders |
| **SOC-A2-F7** Let friends know you are safe | In an earthquake, flood or other emergency in your area, mark yourself safe so friends and family know. | later | P1 Safety check; P2 Emergency source research; P3 Emergency light mode |
| **SOC-A2-F8** Meet new people | Opt into a separate dating space with its own profile, shown only to other adults who opted in. | later | P1 Dating decision; P2 Dating profiles and matches |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A2-F1-P1 | **Social graph store**: Follows, friendships, lists, blocks and mutes for millions of members, fast to read both ways. | data | large | kit-store, kit-core |  |
| SOC-A2-F1-P2 | **Follow button and lists**: Follow, unfollow and follow-back buttons, and searchable followers and following lists. | interface | small | SOC-A2-F1-P1 |  |
| SOC-A2-F1-P3 | **Private accounts**: Members who choose a private account approve each follower first. | service | small | SOC-A2-F1-P1 |  |
| SOC-A2-F2-P1 | **Friend requests**: Two-way friendship record with request, accept, decline and unfriend. | data | small | SOC-A2-F1-P1 |  |
| SOC-A2-F2-P2 | **Requests inbox**: One place for friend and follow requests, with filters for people you may know. | interface | small | SOC-A2-F2-P1 |  |
| SOC-A2-F3-P1 | **Custom lists**: Private lists of people that can be used as a post audience or a feed. | data | small | SOC-A2-F1-P1, SOC-A3-F2-P1 |  |
| SOC-A2-F3-P2 | **List manager**: Add and remove people from lists quickly, including from their profile. | interface | small | SOC-A2-F3-P1 |  |
| SOC-A2-F4-P1 | **Suggestion rules**: Suggestions only from published signals such as mutual follows and shared groups, obeying each member's discoverability choices and the teen contact limits. | service | medium | SOC-A2-F1-P1, SOC-A19-F3-P2, SOC-A14-F2-P2 | [M05](STATUS.md) |
| SOC-A2-F4-P2 | **Why this suggestion**: A reason on every suggestion and a switch to stop being suggested to others. | interface | small | SOC-A2-F4-P1 |  |
| SOC-A2-F5-P1 | **Remove follower and hide follower counts**: Quietly remove a follower, and choose whether your follower and following counts show; like and view counts are hidden with SOC-A14-F4-P3. | service | small | SOC-A2-F1-P1 |  |
| SOC-A2-F6-P1 | **Birthday sharing**: Choose who sees your birthday, separate from the private date of birth used for age rules. | data | small | SOC-A1-F2-P1 |  |
| SOC-A2-F6-P2 | **Reminders**: Opt-in reminders for friends' birthdays and your own posting anniversaries. | service | small | KIT-A8-F3-P2 |  |
| SOC-A2-F7-P1 | **Safety check**: An emergency check-in that friends can see, opened for an area when an emergency is declared there. | service | medium | KIT-A7-F1-P2, SOC-A2-F2-P1 |  |
| SOC-A2-F7-P2 | **Emergency source research**: How to learn of declared emergencies from official sources in each country, starting with New Zealand's, whose terms allow it. | research | small |  |  |
| SOC-A2-F7-P3 | **Emergency light mode**: A text-only, low-data mode for the safety check and posts in a declared emergency, for when networks are overloaded, as they were in Cyclone Gabrielle. | library | small | SOC-A2-F7-P1, KIT-A19-F5-P1 |  |
| SOC-A2-F8-P1 | **Dating decision**: Decide whether dating belongs in Spare Social, in its own program or nowhere, given its own matching, identity checks, scam risks and legal duties. | research | medium |  | [M02](STATUS.md) |
| SOC-A2-F8-P2 | **Dating profiles and matches**: A separate opt-in profile, mutual matching and a chat that opens only on a match, built only if the dating decision says so. | service | large | KIT-A6-F3-P3, SOC-A9-F1-P1, SOC-A2-F8-P1 |  |

## SOC-A3 Posts and conversation

Writing posts with text, links, photos and videos to the audience you choose, then reacting, commenting, reposting, saving, tagging, polling and adding context together.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A3-F1** Write a post | Post text, links, photos or videos, with mentions, tags, a content label and an honest AI-use answer. | core | P1 Post record; P2 Composer; P3 Rich text; P4 Link previews without tracking; P5 Content labels on posts; P7 Post language |
| **SOC-A3-F2** Choose who sees each post | Post to everyone, followers, friends, close friends, a list, a group, paying members or only yourself. | core | P1 Audience rules engine; P2 Audience picker; P3 Audiences on the open network |
| **SOC-A3-F3** Edit, draft, schedule and archive | Fix a post with the edits shown, keep drafts on every device, schedule posts and hide old posts without deleting them. | core | P1 Edits with visible history; P2 Drafts on every device; P3 Scheduled posts; P4 Archive and delete; P5 Remove a past version; P6 Recently deleted |
| **SOC-A3-F4** React and comment | Like or react to a post, comment, reply in threads, and pin or approve comments on your own posts. | core | P1 Reactions; P2 Threaded comments; P3 Comment controls; P4 Pin and highlight; P5 Comment order with published rules |
| **SOC-A3-F5** Share, repost and quote | Repost to your followers, quote with your own words, send to a friend, or share a clean link outside. | core | P1 Repost and quote; P2 Send in a message; P3 Share outside |
| **SOC-A3-F6** Save and collect | Save posts into private collections, or make a collection to share with friends. | core | P1 Saved collections; P2 Shared collections |
| **SOC-A3-F7** Polls and questions | Ask a question with a poll and see the results when it closes. | next | P1 Poll post type; P2 Poll results |
| **SOC-A3-F8** Tag people, places and topics | Tag people who approve it, tag a place on an open map, and add topic tags. | core | P1 People tags with approval; P2 Place tags; P3 Topic tags |
| **SOC-A3-F9** Add context to a post | Write a note that adds missing context to a post; it shows when people who usually disagree both find it helpful. | next | P1 Context notes; P2 Note-ranking licence check |
| **SOC-A3-F10** Post together | Make a post with another person or page so it shows on both profiles with both names. | next | P1 Co-authored posts |
| **SOC-A3-F11** Write long articles | Write a long article with headings, pictures and videos inside it, readable on other open networks too. | later | P1 Article post type; P2 Article editor |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A3-F1-P1 | **Post record**: One record for every kind of post (text, photo, video, story, poll, event share) with its edit history, stored as operations. | data | large | kit-core, kit-ops |  |
| SOC-A3-F1-P2 | **Composer**: Write, attach media, pick the audience, add labels, answer the AI-use question and preview before posting, on phone and web. | interface | medium | SOC-A3-F1-P1, SOC-A17-F1-P1, SOC-A11-F1-P2 |  |
| SOC-A3-F1-P3 | **Rich text**: Mentions, tags, links and emoji, shown safely so no post can run code on a reader's screen; a candidate kit library, since every app shows text people write. | library | medium | engine-text |  |
| SOC-A3-F1-P4 | **Link previews without tracking**: Uses the kit's link previews (KIT-A8-F1-P4) for posts and comments, fetched through the safe fetcher (SOC-A17-F7-P2), so readers are never tracked by the linked site; never used for encrypted chats (SOC-A9-F1-P6). | integration | small | SOC-A3-F1-P1, KIT-A8-F1-P4, SOC-A17-F7-P2 |  |
| SOC-A3-F1-P5 | **Content labels on posts**: The poster can mark a post as sensitive, a spoiler, or graphic, so viewers can choose to blur it. | data | small | SOC-A12-F3-P1 | [M04](STATUS.md) |
| SOC-A3-F1-P7 | **Post language**: Each text post carries its language, detected on the device and changeable by the poster, for language filters and search. | data | small | SOC-A3-F1-P1 |  |
| SOC-A3-F2-P1 | **Audience rules engine**: Checks every read of every post against its audience everywhere Spare Social serves it: feeds, search, links, exports and the API. | library | medium | kit-core, SOC-A2-F1-P1 |  |
| SOC-A3-F2-P2 | **Audience picker**: Pick the audience per post, with your usual choice remembered. | interface | small | SOC-A3-F2-P1 |  |
| SOC-A3-F2-P3 | **Audiences on the open network**: Posts for friends, close friends, lists, paying members or teens leave Spare Social only where the protocol can keep the audience; otherwise they stay here, and the audience picker says so. | library | medium | SOC-A3-F2-P1, SOC-A18-F1-P5, SOC-A18-F1-P6 | [M01](STATUS.md) |
| SOC-A3-F3-P1 | **Edits with visible history**: An edited post says so and anyone who can see it can view earlier versions. | data | small | kit-ops, SOC-A3-F1-P1 |  |
| SOC-A3-F3-P2 | **Drafts on every device**: Unfinished posts sync across phone, web and computer. | data | small | kit-sync |  |
| SOC-A3-F3-P3 | **Scheduled posts**: Pick a time and the post goes out then, in the member's time zone. | service | small | kit-ops |  |
| SOC-A3-F3-P4 | **Archive and delete**: Hide a post from your profile but keep it, or delete it everywhere; deleted posts wait in the recently deleted bin first (SOC-A3-F3-P6). | service | small | SOC-A17-F6-P1, SOC-A3-F3-P6 |  |
| SOC-A3-F3-P5 | **Remove a past version**: The poster, or a moderator for a legal reason logged in public, can remove a past version from the visible edit history. | service | small | SOC-A3-F3-P1, SOC-A13-F5-P1 |  |
| SOC-A3-F3-P6 | **Recently deleted**: Deleted posts and stories wait in a private bin for a published number of days, restored on request, then purged on schedule, using the kit's recycle bin (KIT-A15-F9-P1). | integration | small | KIT-A15-F9-P1, SOC-A17-F6-P1 |  |
| SOC-A3-F4-P1 | **Reactions**: A like plus a small set of reactions, counted per post and per comment. | data | small | kit-ops |  |
| SOC-A3-F4-P2 | **Threaded comments**: Comments with replies in threads, mentions, and photos or short clips in replies. | data | medium | SOC-A3-F1-P1 |  |
| SOC-A3-F4-P3 | **Comment controls**: Choose who can comment, turn comments off, approve before showing, or hide a comment. | service | small | SOC-A3-F4-P2 |  |
| SOC-A3-F4-P4 | **Pin and highlight**: Pin a comment to the top and mark comments the creator liked. | interface | small | SOC-A3-F4-P2 |  |
| SOC-A3-F4-P5 | **Comment order with published rules**: Sort comments by newest, oldest or most replies, with the rule for each shown. | interface | small | SOC-A3-F4-P2 | [M05](STATUS.md) |
| SOC-A3-F5-P1 | **Repost and quote**: Repost or quote a post, keeping the credit and the original audience rules. | data | small | SOC-A3-F1-P1, SOC-A3-F2-P1 |  |
| SOC-A3-F5-P2 | **Send in a message**: Send a post into a chat; the card respects the post's audience. | integration | small | SOC-A9-F5-P1 |  |
| SOC-A3-F5-P3 | **Share outside**: Copy a link, use the phone's share sheet, and give other sites a clean preview card. | service | small | SOC-A3-F1-P1 |  |
| SOC-A3-F6-P1 | **Saved collections**: Private named collections of saved posts. | data | small | SOC-A3-F1-P1 |  |
| SOC-A3-F6-P2 | **Shared collections**: Collections that friends can add to together. | data | small | SOC-A3-F6-P1 |  |
| SOC-A3-F7-P1 | **Poll post type**: One vote per account, cast only by a person, never an agent (SOC-A23-F2-P5), with a closing time and optional hidden results until it closes. | data | small | SOC-A3-F1-P1, SOC-A23-F2-P5 |  |
| SOC-A3-F7-P2 | **Poll results**: Results shown as plain bars with counts. | interface | small | SOC-A3-F7-P1 |  |
| SOC-A3-F8-P1 | **People tags with approval**: Tags of a person show on their profile only after they approve; what is learned about people from others' posts follows the indirect collection rules (SOC-A19-F4-P4). | data | small | SOC-A3-F1-P1, SOC-A19-F4-P4 |  |
| SOC-A3-F8-P2 | **Place tags**: Tag a place using open map data, with an exact or rough location. | integration | small | KIT-A7-F2-P2 |  |
| SOC-A3-F8-P3 | **Topic tags**: Tags that link a post to a topic page. | data | small | SOC-A10-F5-P1 |  |
| SOC-A3-F9-P1 | **Context notes**: Writing and rating notes, shown only when raters from different viewpoints agree, by a published method; rating is a people-only action, never done by an agent (SOC-A23-F2-P5). | service | large | SOC-A3-F1-P1, SOC-A23-F2-P5 | [M04](STATUS.md) |
| SOC-A3-F9-P2 | **Note-ranking licence check**: Check the licence of existing open note-ranking code and the papers behind it before any use. | research | small |  | [M08](STATUS.md) |
| SOC-A3-F10-P1 | **Co-authored posts**: A post with several authors who each accept, shown on every author's profile. | data | small | SOC-A3-F1-P1 |  |
| SOC-A3-F11-P1 | **Article post type**: Long articles with headings, inline media and a summary, sent out as the ActivityPub Article type. | data | small | SOC-A3-F1-P1, SOC-A18-F1-P5 |  |
| SOC-A3-F11-P2 | **Article editor**: Write and lay out a long article with headings, pictures and videos, on phone and web. | interface | medium | SOC-A3-F11-P1, SOC-A3-F1-P3 |  |

## SOC-A4 Photos and stories

Photo posts and swipe-through carousels, a photo editor with our own looks, stories that disappear after a day with interactive stickers, highlights, a profile grid, short status notes and shared albums.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A4-F1** Post photos and carousels | Post one photo or a swipe-through set of photos and clips, cropped, ordered and described. | core | P1 Carousel post type; P2 Picker, order and crop |
| **SOC-A4-F2** Edit photos | Crop, straighten, adjust light and colour, and apply looks designed by the co-op. | core | P1 Photo editor engine; P2 Our own looks; P3 Editor screens |
| **SOC-A4-F3** Share stories that disappear | Post photos, clips and text that vanish after 24 hours, add stickers, and see who watched. | core | P1 Story record; P2 Story composer; P3 Interactive stickers; P4 Story viewer; P5 On-time expiry |
| **SOC-A4-F4** Keep stories as highlights | Keep chosen stories on your profile as named highlights, and look back at your private archive. | next | P1 Highlights; P2 Private archive and memories |
| **SOC-A4-F5** Arrange your profile grid | See your photos as a grid, reorder it and pin favourites. | next | P1 Grid with reorder |
| **SOC-A4-F6** Short status notes | Leave a one-line note that friends see for a day at the top of messages. | later | P1 Status notes |
| **SOC-A4-F7** Shared albums | Make an album that friends or a group can add photos to together. | later | P1 Shared album |
| **SOC-A4-F8** Your places map | See your own tagged photos on a private map. | later | P1 Personal places map |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A4-F1-P1 | **Carousel post type**: Up to a set number of photos and clips in one post, each with its own alt text and tags. | data | small | SOC-A3-F1-P1, SOC-A21-F2-P1 |  |
| SOC-A4-F1-P2 | **Picker, order and crop**: Pick many photos, drag to reorder, and crop to square, portrait or landscape. | interface | medium | SOC-A17-F3-P1 |  |
| SOC-A4-F2-P1 | **Photo editor engine**: Crop, rotate, exposure, contrast, colour and sharpening, run on the device. | library | medium | engine-render, engine-media |  |
| SOC-A4-F2-P2 | **Our own looks**: Filter presets designed by members and the co-op, never copied from other apps. | data | small | SOC-A4-F2-P1 |  |
| SOC-A4-F2-P3 | **Editor screens**: Simple sliders and before-and-after view on phone and web. | interface | medium | SOC-A4-F2-P1 |  |
| SOC-A4-F3-P1 | **Story record**: Stories with an expiry time, audience, and a viewer list only the poster sees. | data | medium | kit-core, SOC-A3-F2-P1 |  |
| SOC-A4-F3-P2 | **Story composer**: Camera, text, drawing, stickers and music on a full-screen canvas. | interface | large | engine-draw2d, engine-text, SOC-A5-F2-P1 |  |
| SOC-A4-F3-P3 | **Interactive stickers**: Poll, question, slider, countdown, link, mention, place and music stickers, with replies to the poster. | data | medium | SOC-A4-F3-P1 |  |
| SOC-A4-F3-P4 | **Story viewer**: Tap through, hold to pause, swipe between people, and reply in a message. | interface | medium | SOC-A4-F3-P1, SOC-A9-F1-P1 |  |
| SOC-A4-F3-P5 | **On-time expiry**: Remove expired stories everywhere on time, keeping a private archive only if the poster chose it. | service | small | SOC-A17-F6-P1 |  |
| SOC-A4-F4-P1 | **Highlights**: Named sets of past stories pinned to the profile. | data | small | SOC-A4-F3-P1 |  |
| SOC-A4-F4-P2 | **Private archive and memories**: Your old stories and posts, with an opt-in look back on this day. | service | small | SOC-A4-F3-P1 |  |
| SOC-A4-F5-P1 | **Grid with reorder**: A profile grid you can rearrange without changing post dates. | interface | small | SOC-A1-F2-P2 |  |
| SOC-A4-F6-P1 | **Status notes**: Short notes with an audience and a 24-hour expiry. | data | small | SOC-A3-F2-P1 |  |
| SOC-A4-F7-P1 | **Shared album**: An album with contributors, each owning their own photos. | data | small | SOC-A3-F1-P1 |  |
| SOC-A4-F8-P1 | **Personal places map**: A map of your tagged posts, private unless you choose. | interface | medium | KIT-A7-F1-P2, SOC-A3-F8-P2 |  |

## SOC-A5 Short videos

The full-screen vertical video feed, the in-app camera and editor, sounds and music, remixing others' videos with credit, series, templates, and cutting shorts from long videos.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A5-F1** Watch a short video feed | Swipe through full-screen vertical videos that start instantly, loop, and can be liked, commented on and shared. | core | P1 Vertical swipe player; P2 Preloading rules; P3 Playback controls |
| **SOC-A5-F2** Record in the app | Film several clips with a timer, speed setting, flip and flash, hands-free. | core | P1 Camera capture; P2 Clip timeline |
| **SOC-A5-F3** Edit a short video | Trim, split, add text, stickers, voiceover, effects, transitions and captions before posting. | core | P1 Short video editor; P2 Text and sticker overlays; P3 Voiceover and mixing; P4 Effects and backgrounds; P5 Final render; P6 Auto captions in the editor |
| **SOC-A5-F4** Add sounds and music | Use your own sound, reuse another video's sound if its creator allows, or pick licensed or free music. | core | P1 Sounds library; P2 Sound pages; P3 Music licensing; P4 Free music collection |
| **SOC-A5-F5** Remix with credit | Reply to a video with your own, side by side or by clipping part of it first, with the original always credited. | next | P1 Remix permissions; P2 Remix layouts; P3 Credit chain; P4 Download switch with credit |
| **SOC-A5-F6** Series and templates | Link short videos into numbered parts, and reuse an edit layout someone shared. | next | P1 Series; P2 Edit templates |
| **SOC-A5-F7** Cut a short from a long video | Pick a moment from your long video and post it as a vertical short that links back. | next | P1 Clip to short |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A5-F1-P1 | **Vertical swipe player**: Full-screen player with swipe, loop, double-tap to like, sound toggle and comments over the video. | interface | large | SOC-A17-F2-P3 |  |
| SOC-A5-F1-P2 | **Preloading rules**: Load the next few videos ahead, within data saver limits. | library | medium | SOC-A20-F4-P3 |  |
| SOC-A5-F1-P3 | **Playback controls**: Speed, scrubbing, captions and an auto-scroll switch. | interface | small | SOC-A5-F1-P1 |  |
| SOC-A5-F2-P1 | **Camera capture**: Record clips on phone and web with timer, speed, flip, flash and a grid. | interface | large | engine-media | [K04](../kit/STATUS.md) |
| SOC-A5-F2-P2 | **Clip timeline**: Keep recorded clips in order, retake or delete one, and see the total length. | library | medium | engine-media |  |
| SOC-A5-F3-P1 | **Short video editor**: Trim, split, speed and order on the shared video engine, on the device. | library | large | engine-media |  |
| SOC-A5-F3-P2 | **Text and sticker overlays**: Timed text, stickers and drawings over the video. | interface | medium | engine-text, engine-draw2d |  |
| SOC-A5-F3-P3 | **Voiceover and mixing**: Record a voiceover and balance it against the original sound and music. | library | medium | engine-media |  |
| SOC-A5-F3-P4 | **Effects and backgrounds**: Effects, transitions and background replacement designed by the co-op and run on the device; anything made with AI is marked as such (SOC-A11-F1-P3). | library | large | engine-render, engine-media, SOC-A11-F1-P3 |  |
| SOC-A5-F3-P5 | **Final render**: Render on the device, with a server fallback for older phones. | service | medium | engine-media, SOC-A17-F2-P6 |  |
| SOC-A5-F3-P6 | **Auto captions in the editor**: Generate captions, then fix the words before posting. | interface | small | SOC-A17-F5-P1 |  |
| SOC-A5-F4-P1 | **Sounds library**: Every video's sound can become a reusable sound if the creator allows it, with credit. | data | medium | SOC-A3-F1-P1 |  |
| SOC-A5-F4-P2 | **Sound pages**: A page for each sound showing every video that uses it. | interface | small | SOC-A5-F4-P1 |  |
| SOC-A5-F4-P3 | **Music licensing**: What licences are needed in each country, starting with New Zealand, for members to use commercial music, and what they cost at cost. | legal | large |  | [M02](STATUS.md) |
| SOC-A5-F4-P4 | **Free music collection**: Music members and artists offer under open licences, with the licence shown. | data | medium | SOC-A5-F4-P1 |  |
| SOC-A5-F5-P1 | **Remix permissions**: Per-video switches for side-by-side, clip-and-reply and sound reuse. | data | small | SOC-A3-F1-P1 |  |
| SOC-A5-F5-P2 | **Remix layouts**: Side-by-side, top-and-bottom and clip-then-respond layouts. | library | medium | engine-media |  |
| SOC-A5-F5-P3 | **Credit chain**: Every remix links back to its source and the source lists its remixes. | data | small | SOC-A5-F5-P1 |  |
| SOC-A5-F5-P4 | **Download switch with credit**: A per-video switch that lets others save the video to their device, with the creator's handle and a link added to the saved copy. | library | small | SOC-A5-F5-P1, engine-media |  |
| SOC-A5-F6-P1 | **Series**: An ordered set of videos with part numbers and a next-part button. | data | small | SOC-A3-F1-P1 |  |
| SOC-A5-F6-P2 | **Edit templates**: Share an edit's timing and layout so others can drop in their own clips. | data | medium | SOC-A5-F3-P1 |  |
| SOC-A5-F7-P1 | **Clip to short**: Choose a range, reframe it to vertical and post it with a link to the full video. | library | medium | engine-media, SOC-A6-F1-P1 |  |

## SOC-A6 Long videos and channels

Uploading and watching long videos, channels and subscriptions, playlists and history, captions in many languages, chapters and links inside videos, premieres, and immersive video.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A6-F1** Upload a long video | Upload a video with a title, description, thumbnail, language, licence and visibility, and release it now or later. | core | P1 Video details editor; P2 Thumbnails; P3 Licence choice; P4 Visibility and release; P5 Made for children |
| **SOC-A6-F2** Watch a video | Watch in any quality with speed, captions, chapters, full screen, picture-in-picture and keyboard shortcuts, picking up where you left off. | core | P1 Video player; P2 Chapters; P3 Resume anywhere; P4 Picture-in-picture and background play; P5 Timestamps in comments |
| **SOC-A6-F3** Channels and subscriptions | Visit a channel with its videos, playlists and posts, subscribe, and choose how much to be notified. | core | P1 Channel page; P2 Subscribe and notify setting; P3 Subscriptions feed; P4 Channel web feeds |
| **SOC-A6-F4** Playlists, watch later and history | Make playlists, save videos for later, see what you watched, and pause or clear your history. | core | P1 Playlists; P2 Watch later and history; P3 Queue and autoplay |
| **SOC-A6-F5** Captions and subtitles in many languages | Add captions, upload subtitle files, accept members' translations, and offer dubbed audio. | core | P1 Caption editor and upload; P2 Community translations; P3 Extra audio tracks; P4 Translated titles and descriptions |
| **SOC-A6-F6** Links inside videos | Add timed cards and an end screen that point to other videos, playlists or websites. | next | P1 Timed link cards; P2 End screens |
| **SOC-A6-F7** Premieres | Release a new video at a set time and watch it together with live chat. | later | P1 Premiere countdown and chat |
| **SOC-A6-F8** Immersive video | Watch 360-degree, virtual reality and high dynamic range videos. | later | P1 360 and VR playback; P2 HDR delivery |
| **SOC-A6-F9** Podcasts | Publish an audio show in episodes, or bring one in from its RSS feed, and listen with the screen off. | later | P1 Podcast import from RSS; P2 Audio-only listening |
| **SOC-A6-F10** Fix a published video | Trim, cut out a section, blur faces or objects, or replace the file of a published video without losing its comments and links. | next | P1 Trim and cut after publishing; P2 Blur faces and objects; P3 Replace the file |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A6-F1-P1 | **Video details editor**: Title, description, tags, language, category, AI-use answer and audience, editable after posting. | interface | medium | SOC-A17-F1-P1, SOC-A3-F1-P1 |  |
| SOC-A6-F1-P2 | **Thumbnails**: Upload a custom thumbnail or pick a frame from the video. | interface | small | SOC-A17-F2-P4 |  |
| SOC-A6-F1-P3 | **Licence choice**: All rights reserved or a Creative Commons licence, shown under the video. | data | small | SOC-A6-F1-P1 |  |
| SOC-A6-F1-P4 | **Visibility and release**: Public, unlisted, private, members only, or scheduled for a set time. | data | small | SOC-A3-F2-P1 |  |
| SOC-A6-F1-P5 | **Made for children**: A flag for videos made for children that turns off comments, watch-signal collection and personalised discovery for anyone watching them, as US COPPA expects once US children can watch. | data | small | SOC-A6-F1-P1, SOC-A3-F4-P3 |  |
| SOC-A6-F2-P1 | **Video player**: Our own player with adaptive streaming, quality and speed menus, captions and keyboard control. | interface | large | SOC-A17-F2-P3 |  |
| SOC-A6-F2-P2 | **Chapters**: Chapters from timestamps in the description, shown on the progress bar. | data | small | SOC-A6-F1-P1 |  |
| SOC-A6-F2-P3 | **Resume anywhere**: Your place in each video syncs across devices. | data | small | kit-sync |  |
| SOC-A6-F2-P4 | **Picture-in-picture and background play**: Keep watching in a small window or listen with the screen off. | interface | small | SOC-A6-F2-P1 | [K04](../kit/STATUS.md) |
| SOC-A6-F2-P5 | **Timestamps in comments**: Times written in comments jump the player to that moment. | interface | small | SOC-A3-F4-P2 |  |
| SOC-A6-F3-P1 | **Channel page**: A profile laid out for video, with sections the creator arranges. | interface | medium | SOC-A1-F2-P2 |  |
| SOC-A6-F3-P2 | **Subscribe and notify setting**: Subscribe with notifications for all, some or no uploads. | data | small | SOC-A2-F1-P1, svc-messaging |  |
| SOC-A6-F3-P3 | **Subscriptions feed**: New uploads from your subscriptions, newest first. | service | small | SOC-A10-F1-P1 |  |
| SOC-A6-F3-P4 | **Channel web feeds**: An RSS or Atom feed for each channel, so people can follow without an account. | service | small | SOC-A6-F3-P1 |  |
| SOC-A6-F4-P1 | **Playlists**: Public, unlisted, private and shared playlists that others can add to. | data | small | SOC-A3-F1-P1 |  |
| SOC-A6-F4-P2 | **Watch later and history**: A watch-later list, and a history you can pause, search and clear. | data | small | kit-sync |  |
| SOC-A6-F4-P3 | **Queue and autoplay**: Queue videos to play next; autoplay follows the one autoplay setting (SOC-A21-F4-P1), off unless you turn it on, with its rule shown. | interface | small | SOC-A6-F2-P1, SOC-A21-F4-P1 | [M05](STATUS.md) |
| SOC-A6-F5-P1 | **Caption editor and upload**: Edit automatic captions or upload WebVTT and SRT files. | interface | medium | SOC-A17-F5-P2 |  |
| SOC-A6-F5-P2 | **Community translations**: Members suggest subtitles in other languages; the creator approves them. | service | medium | SOC-A6-F5-P1 |  |
| SOC-A6-F5-P3 | **Extra audio tracks**: Several audio languages on one video, chosen in the player; tracks dubbed with AI carry a label and content credentials (SOC-A11-F1-P3). | library | medium | engine-media, SOC-A11-F1-P3 |  |
| SOC-A6-F5-P4 | **Translated titles and descriptions**: Titles, descriptions and chapter names in other languages, shown in each viewer's language and found by search. | data | small | SOC-A6-F1-P1, SOC-A6-F2-P2 |  |
| SOC-A6-F6-P1 | **Timed link cards**: Cards that appear at chosen times and link to a video, playlist, post or web address. | data | small | SOC-A6-F1-P1 |  |
| SOC-A6-F6-P2 | **End screens**: Suggested next videos chosen by the creator, shown in the last seconds. | interface | small | SOC-A6-F2-P1 |  |
| SOC-A6-F7-P1 | **Premiere countdown and chat**: A waiting page with a countdown, then the video plays for everyone with live chat. | service | medium | SOC-A7-F2-P1, SOC-A6-F1-P4 |  |
| SOC-A6-F8-P1 | **360 and VR playback**: Look around inside a 360 video on phone, computer and headset. | interface | large | engine-render |  |
| SOC-A6-F8-P2 | **HDR delivery**: Research how to keep high dynamic range from upload to screen. | research | medium | engine-media | [S03](../spare-studio/STATUS.md) |
| SOC-A6-F9-P1 | **Podcast import from RSS**: Import a show and its new episodes from its public RSS feed, once the owner proves it is theirs. | service | medium | SOC-A17-F1-P1 |  |
| SOC-A6-F9-P2 | **Audio-only listening**: Listen to shows and videos as audio only, saving data. | interface | small | SOC-A6-F2-P4 |  |
| SOC-A6-F10-P1 | **Trim and cut after publishing**: Trim the ends or cut a section out of a published video and process it again, keeping its link, comments and stats, with the change shown in the video's history. | service | medium | SOC-A17-F2-P1, SOC-A6-F1-P1, SOC-A17-F1-P4 |  |
| SOC-A6-F10-P2 | **Blur faces and objects**: Pick faces, number plates or objects to blur through the video, followed from frame to frame, the fix for privacy complaints; face finding meets the biometric code (SOC-A14-F1-P5). | library | medium | engine-media, SOC-A6-F10-P1, SOC-A14-F1-P5 |  |
| SOC-A6-F10-P3 | **Replace the file**: Swap in a new file for a published video, keeping its link and comments, with the swap noted on the video. | service | small | SOC-A6-F10-P1 |  |

## SOC-A7 Live streaming

Going live from a phone or streaming software, live chat with moderation, guests, replays and clips, audio rooms, and the fast safety response live video needs.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A7-F1** Go live | Start a live stream from your phone or from streaming software on a computer. | later | P1 Stream intake; P2 Live transcoding; P3 Low-delay delivery; P4 Go-live screen |
| **SOC-A7-F2** Live chat | Chat with viewers in real time, with slow mode, follower-only chat, word filters and helpers who moderate. | later | P1 Live chat service; P2 Chat moderation tools |
| **SOC-A7-F3** Bring guests on | Invite other people onto your stream side by side. | later | P1 Multi-guest mixing |
| **SOC-A7-F4** Replays and clips | Keep the stream as a video afterwards, and let viewers clip a moment to share. | later | P1 Save as video; P2 Clip a moment |
| **SOC-A7-F5** Live audio rooms | Host an audio-only room with speakers and listeners who raise a hand to talk. | later | P1 Audio rooms |
| **SOC-A7-F6** Live safety | Report a live stream as it happens and have it stopped fast when it breaks the law. | later | P1 Real-time reports and stop; P2 Who can go live |
| **SOC-A7-F7** Tips during a live stream | Viewers send a tip that shows on screen during the stream. | later | P1 Live tip alerts |
| **SOC-A7-F8** Live captions | Read captions as people speak in live streams and audio rooms. | later | P1 Captions for lives and audio rooms |
| **SOC-A7-F9** Plan and wrap up a live | Announce a live ahead with a waiting page, let viewers rewind while it runs, and send them on to another channel at the end. | later | P1 Scheduled lives; P2 Rewind while live; P3 Send viewers on |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A7-F1-P1 | **Stream intake**: Accept streams from phones over WebRTC (WHIP) and from streaming software over RTMP and SRT. | service | large | engine-media | [M03](STATUS.md) |
| SOC-A7-F1-P2 | **Live transcoding**: Turn each stream into several sizes as it arrives. | service | large | SOC-A17-F2-P1 | [M03](STATUS.md) |
| SOC-A7-F1-P3 | **Low-delay delivery**: Compare low-latency HLS and WebRTC delivery on delay, cost and scale. | research | medium | SOC-A17-F4-P2 | [M06](STATUS.md) |
| SOC-A7-F1-P4 | **Go-live screen**: Title, audience, AI-use answer and a camera check before going live. | interface | medium | SOC-A5-F2-P1 |  |
| SOC-A7-F2-P1 | **Live chat service**: Real-time chat that stays fast with many thousands of viewers, built on the kit's chat service (KIT-A8-F1-P2); a candidate kit part, since other apps' live events need it. | service | large | KIT-A8-F1-P2 |  |
| SOC-A7-F2-P2 | **Chat moderation tools**: Slow mode, followers-only, hidden words, timeouts and chat moderators. | service | medium | SOC-A7-F2-P1, SOC-A12-F2-P1 |  |
| SOC-A7-F3-P1 | **Multi-guest mixing**: Mix several cameras and microphones into one stream. | service | large | engine-media, SOC-A9-F4-P1 |  |
| SOC-A7-F4-P1 | **Save as video**: Save the stream with its chat replay as an ordinary video. | service | medium | SOC-A6-F1-P1 |  |
| SOC-A7-F4-P2 | **Clip a moment**: Viewers cut a short clip from a live stream or replay, with credit. | library | medium | engine-media, SOC-A5-F5-P3 |  |
| SOC-A7-F5-P1 | **Audio rooms**: Rooms with hosts, speakers and listeners, with recording if the host allows. | service | large | SOC-A9-F4-P1 |  |
| SOC-A7-F6-P1 | **Real-time reports and stop**: A fast report path and a moderator switch that ends a stream at once. | service | medium | svc-trust, SOC-A13-F1-P1, SOC-A13-F3-P10 | [M07](STATUS.md) |
| SOC-A7-F6-P2 | **Who can go live**: Published rules on who can go live, informed by the duty to act fast on violent extremist live video. | legal | small |  | [M02](STATUS.md) |
| SOC-A7-F7-P1 | **Live tip alerts**: Tips shown on stream with the payment fee shown to the viewer. | integration | small | SOC-A15-F1-P1 |  |
| SOC-A7-F8-P1 | **Captions for lives and audio rooms**: Captions made as people speak in live streams and audio rooms, using the kit's captions (KIT-A19-F4-P1). | service | medium | KIT-A19-F4-P1, SOC-A7-F1-P1, SOC-A7-F5-P1 |  |
| SOC-A7-F9-P1 | **Scheduled lives**: Announce a live for a set time with a waiting page, a countdown and reminders. | service | small | SOC-A7-F1-P4, svc-messaging |  |
| SOC-A7-F9-P2 | **Rewind while live**: Viewers rewind a live stream while it is still running, within a window the streamer sets. | library | medium | SOC-A7-F1-P2, SOC-A17-F2-P3 |  |
| SOC-A7-F9-P3 | **Send viewers on**: At the end of a stream, send viewers to another channel's live stream if that channel accepts. | service | small | SOC-A7-F1-P1 |  |

## SOC-A8 Groups, pages and events

Groups of every size and privacy, running a group well, pages for organisations and public figures, events with invites and reminders, fundraisers for charities, and links to Spare Market listings.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A8-F1** Start and join groups | Make a public, private or hidden group, answer join questions, and invite people. | core | P1 Group record; P2 Join requests and invites; P3 Group page |
| **SOC-A8-F2** Run a group | Share the work with admins and moderators, approve posts, set rules, and keep a log members can read. | core | P1 Group roles; P2 Post approval queue; P3 Group moderation log; P4 Elected group admins; P5 Group insights; P6 Admins' legal guide |
| **SOC-A8-F3** Talk in a group | Post, comment and chat in the group, sorted into topics, with shared files and guides. | core | P1 Group topics; P2 Group chat rooms; P3 Files and guides; P4 Anonymous group posts |
| **SOC-A8-F4** Pages for organisations and public figures | Make a page for a business, club, charity, council or public figure with contact details, hours and staff roles. | next | P1 Page profile type; P2 Page roles for staff; P3 Page inbox |
| **SOC-A8-F5** Events | Create an event, invite people, collect replies, send reminders and updates, and add it to your calendar. | next | P1 Event record; P2 Replies and guest list; P3 Reminders and updates; P4 Calendar export; P5 Event map; P6 Events near you; P7 Paid tickets |
| **SOC-A8-F6** Fundraisers | Raise money for a registered charity or a cause, with receipts. | later | P1 Charity fundraisers; P2 Fundraising law; P3 Charity check |
| **SOC-A8-F7** Share Spare Market listings | Share a listing or shop from Spare Market into a post or group; buying happens in Spare Market. | later | P1 Listing cards |
| **SOC-A8-F8** Keep groups and pages going | Hand a group or page to someone else, plan what happens if its owner leaves or dies, settle who controls it, or archive it. | next | P1 Hand over, succession and archive; P2 Control disputes |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A8-F1-P1 | **Group record**: Groups with privacy level, members, rules, and their own posts and media. | data | medium | kit-core, SOC-A3-F2-P1 |  |
| SOC-A8-F1-P2 | **Join requests and invites**: Join questions, approval, invite links and member removal. | service | small | SOC-A8-F1-P1 |  |
| SOC-A8-F1-P3 | **Group page**: Posts, members, about, rules, events and files on one page. | interface | medium | SOC-A8-F1-P1 |  |
| SOC-A8-F2-P1 | **Group roles**: Owner, admin, moderator and member, each with set permissions, built on the kit's roles per app (KIT-A1-F5-P2) and shared with profile and page roles. | library | medium | KIT-A1-F5-P2, kit-core |  |
| SOC-A8-F2-P2 | **Post approval queue**: Hold posts for approval, with keyword alerts for the group's own rules using the one hidden-words library (SOC-A12-F2-P1). | interface | medium | SOC-A8-F2-P1, SOC-A12-F2-P1 |  |
| SOC-A8-F2-P3 | **Group moderation log**: Members can see what the group's moderators removed and why. | service | small | SOC-A13-F5-P1 | [M04](STATUS.md) |
| SOC-A8-F2-P4 | **Elected group admins**: A group can elect its admins with the shared voting service. | integration | medium | svc-voting |  |
| SOC-A8-F2-P5 | **Group insights**: Admins see member growth, active members and post reach as totals only, counted with no per-person tracking. | interface | small | SOC-A16-F1-P1, SOC-A8-F1-P1 |  |
| SOC-A8-F2-P6 | **Admins' legal guide**: Plain guidance for group and page admins on when they can be liable for others' comments they know about, as in New Zealand's Murray v Wishart and Australia's Voller case. | legal | small |  | [M02](STATUS.md) |
| SOC-A8-F3-P1 | **Group topics**: Topics or channels inside a group to sort posts. | data | small | SOC-A8-F1-P1 |  |
| SOC-A8-F3-P2 | **Group chat rooms**: Chats that belong to a group and follow its membership. | integration | small | SOC-A9-F2-P1 |  |
| SOC-A8-F3-P3 | **Files and guides**: Shared files and step-by-step guides inside a group. | data | medium | SOC-A8-F1-P1, SOC-A17-F1-P1 |  |
| SOC-A8-F3-P4 | **Anonymous group posts**: Post in a group without your name showing to members if the group allows it; admins and the law can still see who posted. | data | small | SOC-A8-F1-P1 |  |
| SOC-A8-F4-P1 | **Page profile type**: A profile type with organisation details, opening hours and contact buttons. | data | small | SOC-A1-F4-P1 |  |
| SOC-A8-F4-P2 | **Page roles for staff**: Several staff run one page without sharing a login. | service | small | SOC-A1-F4-P3, KIT-A1-F7-P1 |  |
| SOC-A8-F4-P3 | **Page inbox**: Messages to the page shared between its staff. | interface | medium | SOC-A9-F1-P1 |  |
| SOC-A8-F5-P1 | **Event record**: Times with time zones, a place or online link, hosts, and audience. | data | medium | kit-core, SOC-A3-F2-P1 |  |
| SOC-A8-F5-P2 | **Replies and guest list**: Going, maybe and can't go, with a guest list the host can keep private. | service | small | SOC-A8-F5-P1 |  |
| SOC-A8-F5-P3 | **Reminders and updates**: Reminders before the event and updates to everyone who replied. | service | small | svc-messaging |  |
| SOC-A8-F5-P4 | **Calendar export**: Add events to any calendar with the iCalendar standard, using the kit's calendar import and export (KIT-A5-F5-P2). | library | small | SOC-A8-F5-P1, KIT-A5-F5-P2 |  |
| SOC-A8-F5-P5 | **Event map**: Show the place on an open map with directions. | integration | small | svc-maps |  |
| SOC-A8-F5-P6 | **Events near you**: Find upcoming public events by area and date. | service | medium | svc-maps, SOC-A10-F6-P1 |  |
| SOC-A8-F5-P7 | **Paid tickets**: Sell tickets through the shared payments service. | integration | medium | svc-payments, kit-market | [K02](../kit/STATUS.md) |
| SOC-A8-F6-P1 | **Charity fundraisers**: Donations to registered charities through the payment provider, with donation receipts. | integration | large | svc-payments, SOC-A8-F6-P3 | [K02](../kit/STATUS.md) |
| SOC-A8-F6-P2 | **Fundraising law**: Each country's rules for charitable fundraising, receipts and tax relief, starting with New Zealand's donation tax credits. | legal | medium |  | [M02](STATUS.md) |
| SOC-A8-F6-P3 | **Charity check**: Confirms a charity on its country's charities register (example: New Zealand's Charities Register) before its fundraiser opens, using the kit's charities register check (KIT-A2-F7-P3). | integration | small | KIT-A2-F7-P3 |  |
| SOC-A8-F7-P1 | **Listing cards**: Rich cards for Spare Market listings, drawn from the kit's listing model (KIT-A5-F1-P1), never from Spare Market's own code. | integration | small | KIT-A5-F1-P1, kit-market |  |
| SOC-A8-F8-P1 | **Hand over, succession and archive**: Transfer ownership, name backup owners, pass a group to its admins when the owner leaves or dies, and archive a group so it can still be read. | service | small | SOC-A8-F1-P1, SOC-A8-F2-P1, KIT-A18-F11-P1 |  |
| SOC-A8-F8-P2 | **Control disputes**: When people disagree over who controls a group or page, a member panel decides by published rules. | integration | small | KIT-A11-F1-P1, KIT-A11-F4-P1 |  |

## SOC-A9 Messages and calls

Private one-to-one and group chats with end-to-end encryption, voice notes, disappearing messages, voice and video calls, sharing posts into chats, broadcast channels, safety inside encrypted chats, and the same chats on every device.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A9-F1** Chat one to one | Send texts, photos, videos, voice notes and reactions, reply to a message, edit or unsend, and choose whether to show read receipts. | core | P1 Encrypted messaging; P2 Chat screens; P3 Voice notes; P4 Message requests; P5 Stickers and GIFs; P6 Link previews made on your device; P7 Check who you are talking to; P8 Chat tools, search and files |
| **SOC-A9-F2** Group chats | Chat with a group, name it, add admins, mention people and run quick polls. | core | P1 Encrypted group chats; P2 Group chat admin |
| **SOC-A9-F3** Disappearing messages | Set chats so messages delete themselves after a time you choose. | next | P1 Message timers |
| **SOC-A9-F4** Voice and video calls | Call one person or a group by voice or video, and share your screen. | next | P1 Calls; P2 Call screens; P3 Encrypted group calls; P4 Captions in calls |
| **SOC-A9-F5** Share into chat | Send a post, video, profile or event into a chat and talk about it there. | core | P1 Post cards in chats |
| **SOC-A9-F6** Broadcast channels | Creators send one-way updates to many followers, who can react and vote in polls. | next | P1 Broadcast channel |
| **SOC-A9-F7** Stay safe in messages | Report a chat, block someone, filter requests and blur unwanted images before you see them. | core | P1 Reporting from encrypted chats; P2 On-device image blur; P3 Lawful access and encryption |
| **SOC-A9-F8** Same chats on every device | Use messages on your phone, computer and the web, and restore them on a new phone. | core | P1 Multi-device keys; P2 Encrypted backup |
| **SOC-A9-F9** Chat with other open networks | Message people on other open networks from Spare Social. | later | P1 Messaging bridges |
| **SOC-A9-F10** Watch together | Watch a video together in a chat or call, in step with each other. | later | P1 Synced playback |
| **SOC-A9-F11** Know when an AI reads a chat | Let your own AI read or send in your encrypted chats only from your own device, and see when someone else's AI can read a chat you are in. | core | P1 AI access on your own device; P2 Tell others or refuse |
| **SOC-A9-F12** Chat extras | Share your live location for a set time, send photos that open once, schedule messages, see when a message was forwarded and share contact cards. | later | P1 Live location for a set time; P2 View-once and scheduled messages; P3 Forward labels and contact cards |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A9-F1-P1 | **Encrypted messaging**: Private chats on the kit's chat service (KIT-A8-F1-P2), end-to-end encrypted with the MLS standard (RFC 9420) as set by the kit's encryption decision (KIT-A8-F10-P1). | integration | medium | KIT-A8-F1-P2, KIT-A8-F10-P1 |  |
| SOC-A9-F1-P2 | **Chat screens**: Chat list, chat view, replies, reactions, edit and unsend, typing and read receipts with switches. | interface | large | SOC-A9-F1-P1 |  |
| SOC-A9-F1-P3 | **Voice notes**: Record, send and play voice notes, with optional transcripts made on the device. | library | small | engine-media |  |
| SOC-A9-F1-P4 | **Message requests**: Messages from people you don't follow go to a filtered requests folder. | service | small | SOC-A2-F1-P1 |  |
| SOC-A9-F1-P5 | **Stickers and GIFs**: Co-op and member sticker packs, with GIF search only through a provider whose terms allow it. | interface | small | SOC-A9-F1-P2 |  |
| SOC-A9-F1-P6 | **Link previews made on your device**: In encrypted chats the sender's device fetches a link's preview and sends it inside the encrypted message, so the server never sees the link. | library | small | SOC-A9-F1-P1 |  |
| SOC-A9-F1-P7 | **Check who you are talking to**: Safety numbers to compare in person, and key transparency so a swapped key is caught without anyone checking by hand. | library | medium | SOC-A9-F8-P1 |  |
| SOC-A9-F1-P8 | **Chat tools, search and files**: Mute, pin and archive chats, search inside chats on the device, and send documents as attachments. | interface | medium | SOC-A9-F1-P2, KIT-A15-F7-P1 |  |
| SOC-A9-F2-P1 | **Encrypted group chats**: Group encryption using MLS groups, with members joining and leaving safely; to be built once in the kit beside KIT-A8-F10-P1 and shared with Spare Office. | library | large | SOC-A9-F1-P1, KIT-A8-F10-P1 |  |
| SOC-A9-F2-P2 | **Group chat admin**: Names, photos, admins, invite links and member removal. | interface | small | SOC-A9-F2-P1 |  |
| SOC-A9-F3-P1 | **Message timers**: Per-chat timers enforced on every device in the chat. | data | small | SOC-A9-F1-P1 |  |
| SOC-A9-F4-P1 | **Calls**: One-to-one and group calls on the kit's call service (KIT-A8-F11-P2), with relay servers run at cost. | integration | medium | KIT-A8-F11-P2 |  |
| SOC-A9-F4-P2 | **Call screens**: Ringing, call controls, screen sharing and a call history. | interface | medium | SOC-A9-F4-P1 |  |
| SOC-A9-F4-P3 | **Encrypted group calls**: End-to-end encrypted calls using SFrame (RFC 9605) with keys from the chat's MLS group, and relays that hide each person's IP address from the others. | library | large | SOC-A9-F4-P1, SOC-A9-F2-P1 |  |
| SOC-A9-F4-P4 | **Captions in calls**: Live captions in voice and video calls, made on each person's own device so encrypted calls stay private. | library | medium | SOC-A9-F4-P1, KIT-A19-F4-P1 |  |
| SOC-A9-F5-P1 | **Post cards in chats**: Cards for shared posts that only show if the receiver is allowed to see the post. | library | small | SOC-A3-F2-P1 |  |
| SOC-A9-F6-P1 | **Broadcast channel**: One-way channels with reactions, polls and a members-only option. | service | medium | svc-messaging, SOC-A9-F1-P1 |  |
| SOC-A9-F7-P1 | **Reporting from encrypted chats**: The reporter's own device sends the reported messages to moderators, so encryption stays intact. | library | medium | svc-trust, SOC-A13-F1-P1 | [M07](STATUS.md) |
| SOC-A9-F7-P2 | **On-device image blur**: Sensitive image detection on the device that blurs until you choose to look; on by default for teens. | library | medium | SOC-A14-F2-P1 | [M07](STATUS.md) |
| SOC-A9-F7-P3 | **Lawful access and encryption**: What each country's law asks of an encrypted messaging service, starting with New Zealand's, and what it cannot ask. | legal | medium |  | [M02](STATUS.md) |
| SOC-A9-F8-P1 | **Multi-device keys**: Each device holds its own keys and joins chats securely; to be built in one kit encryption library beside KIT-A8-F10-P1, shared with Spare Office. | library | large | SOC-A9-F1-P1, KIT-A8-F10-P1 |  |
| SOC-A9-F8-P2 | **Encrypted backup**: Backups the co-op can't read, restored with your recovery key; part of the same kit encryption library. | service | medium | SOC-A9-F8-P1, kit-store |  |
| SOC-A9-F9-P1 | **Messaging bridges**: Research joining open messaging networks without weakening encryption. | research | medium | SOC-A18-F1-P1, KIT-A8-F1-P1 | [OF05](../spare-office/STATUS.md) |
| SOC-A9-F10-P1 | **Synced playback**: Play, pause and seek in step for everyone in the chat or call. | service | medium | SOC-A9-F4-P1, SOC-A6-F2-P1 |  |
| SOC-A9-F11-P1 | **AI access on your own device**: An agent can read or send in encrypted chats only through an MCP endpoint on the member's own device, never through the co-op's servers. | library | medium | SOC-A23-F2-P1, SOC-A9-F8-P1 |  |
| SOC-A9-F11-P2 | **Tell others or refuse**: Everyone in a chat sees when someone's AI can read it, and a chat can be set to no AI readers, which Spare Social's own apps and MCP endpoint obey. | service | small | SOC-A9-F11-P1, SOC-A9-F2-P2 |  |
| SOC-A9-F12-P1 | **Live location for a set time**: Share your live location in a chat for a set time, then it stops by itself; never offered to teens. | service | medium | SOC-A9-F1-P1, KIT-A7-F1-P2, SOC-A14-F2-P1 |  |
| SOC-A9-F12-P2 | **View-once and scheduled messages**: Photos that can be opened once, and messages sent at a time you pick. | library | small | SOC-A9-F1-P1 |  |
| SOC-A9-F12-P3 | **Forward labels and contact cards**: Forwarded messages say so, often-forwarded ones go to one chat at a time, and contact cards share a profile in one tap. | library | small | SOC-A9-F1-P1 |  |

## SOC-A10 Feeds, discovery and search

The time-order feed of people you follow, feeds anyone can make and publish with visible rules, a discovery feed whose ranking is published, search across everything, tag and trending pages, local feeds, and your own feed controls.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A10-F1** Following feed in time order | See posts from the people, groups and tags you follow, newest first, with nothing hidden. | core | P1 Timeline builder; P2 Feed screens; P3 Caught-up marker |
| **SOC-A10-F2** Pick or make your own feed | Choose feeds from a directory, see each one's rules, or build one yourself without code. | core | P1 Feed definition format; P2 Feed plugins; P3 Feed directory; P4 No-code feed builder |
| **SOC-A10-F3** Discovery with published rules | Find new videos and people in a discovery feed whose every ranking signal and weight is published. | core | P1 Published discovery ranking; P2 Your signals, your control; P3 Why am I seeing this; P4 Ranking audit tests; P5 Not interested buttons |
| **SOC-A10-F4** Search everything | Search people, posts, videos, sounds, tags, groups, events and places, and words spoken inside videos. | core | P1 Spare Social's search needs; P2 Search on the kit service; P3 Search screens; P4 Search inside videos; P5 Search that ignores macrons; P6 Other servers' posts searched only with consent |
| **SOC-A10-F5** Tags and trending | Open a tag's page to see its newest and most-liked posts, and see what is trending by a published count. | core | P1 Tag pages; P2 Trending with published counting |
| **SOC-A10-F6** Local and country feeds | See posts and events from your town, region or whole country if you share a rough location. | next | P1 Region feeds |
| **SOC-A10-F7** Your feed controls | Hide reposts, pick content types, filter languages and set each feed your way. | core | P1 Per-feed settings |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A10-F1-P1 | **Timeline builder**: Builds each member's time-order feed from follows, groups and tags, fast at scale. | service | large | kit-store, SOC-A2-F1-P1, SOC-A3-F2-P1 | [M05](STATUS.md) |
| SOC-A10-F1-P2 | **Feed screens**: Feeds mixed or split by type (videos, photos, posts), switchable at the top. | interface | medium | SOC-A10-F1-P1 |  |
| SOC-A10-F1-P3 | **Caught-up marker**: Remember where you got to and say when you are all caught up. | data | small | kit-sync |  |
| SOC-A10-F2-P1 | **Feed definition format**: A published format for a feed's rules that anyone can read before choosing it. | data | medium | kit-core | [M05](STATUS.md) |
| SOC-A10-F2-P2 | **Feed plugins**: Custom feed logic runs as sandboxed plugins with declared permissions. | library | large | kit-plugin, SOC-A10-F2-P1 | [A08](../spare-construct/STATUS.md) |
| SOC-A10-F2-P3 | **Feed directory**: Browse, preview and pin feeds, with each feed's rules and maker shown. | interface | medium | SOC-A10-F2-P1 |  |
| SOC-A10-F2-P4 | **No-code feed builder**: Build a feed from people, tags, lengths, languages, places and the human or AI section. | interface | medium | SOC-A10-F2-P1 |  |
| SOC-A10-F3-P1 | **Published discovery ranking**: The default discovery ranking with its signals and weights open for anyone to read and check. | service | large | SOC-A10-F2-P1 | [M05](STATUS.md) |
| SOC-A10-F3-P2 | **Your signals, your control**: The watch and like signals used for you, kept on the device where possible, which you can see, pause and reset. | data | medium | kit-sync | [M05](STATUS.md) |
| SOC-A10-F3-P3 | **Why am I seeing this**: Every post in a ranked feed explains why it is there. | interface | small | SOC-A10-F3-P1 |  |
| SOC-A10-F3-P4 | **Ranking audit tests**: Tests that prove the running ranking matches the published rules. | service | medium | SOC-A10-F3-P1 |  |
| SOC-A10-F3-P5 | **Not interested buttons**: Not interested, show fewer like this, and stop showing this account, applied at once. | interface | small | SOC-A10-F3-P2 |  |
| SOC-A10-F4-P1 | **Spare Social's search needs**: Spare Social's needs for the kit's one search engine choice (KIT-A6-F1-P1): captions, many languages, every post's audience and posts from other servers, decided in the same note. | research | small | KIT-A6-F1-P1 | [MK05](../spare-market/STATUS.md) |
| SOC-A10-F4-P2 | **Search on the kit service**: People, posts, videos, sounds, tags, groups, events and places indexed in the kit's one search service (KIT-A6-F1-P2), with its audience filters (KIT-A6-F1-P4) checking every result against the post's audience. | integration | medium | KIT-A6-F1-P2, KIT-A6-F1-P4, SOC-A3-F2-P1 |  |
| SOC-A10-F4-P3 | **Search screens**: Results by type with filters for date, length, language and section. | interface | medium | SOC-A10-F4-P2 |  |
| SOC-A10-F4-P4 | **Search inside videos**: Search the captions so a video is found by what is said in it. | service | medium | SOC-A17-F5-P1, SOC-A10-F4-P2 |  |
| SOC-A10-F4-P5 | **Search that ignores macrons**: Search ignores macrons and matches te reo Māori spelling forms, using the kit's macron-aware matching (KIT-A19-F1-P4), so Maori finds Māori. | integration | small | KIT-A19-F1-P4, SOC-A10-F4-P2 |  |
| SOC-A10-F4-P6 | **Other servers' posts searched only with consent**: Posts from other servers are indexed for full-text search only where their authors allow it, such as Mastodon's indexable flag, and are dropped from the index when they opt out. | service | small | SOC-A10-F4-P2, SOC-A18-F4-P1 | [M01](STATUS.md) |
| SOC-A10-F5-P1 | **Tag pages**: Newest and most-liked posts for each tag, and a follow button. | interface | small | SOC-A10-F4-P2 |  |
| SOC-A10-F5-P2 | **Trending with published counting**: Trending lists counted by a published rule from people's actions only, never agents' (SOC-A23-F2-P5), with checks against fake boosting. | service | medium | SOC-A13-F8-P2, SOC-A23-F2-P5 | [M05](STATUS.md) |
| SOC-A10-F6-P1 | **Region feeds**: Feeds by town, region and country from rough locations members choose to share. | service | medium | svc-maps, SOC-A10-F1-P1 |  |
| SOC-A10-F7-P1 | **Per-feed settings**: Show or hide reposts, replies, types, languages and sections per feed. | interface | small | SOC-A10-F1-P1, SOC-A11-F2-P1 |  |

## SOC-A11 Human and AI sections

Every feed has a Human section and an AI section. Creators declare AI use and the section follows that answer whatever app or tool posted it, bots and agents are labelled, our own AI tools mark what they make, uploads are checked for content credentials and detection signals as hints only, sections work across the open network, and wrong placements are fixed in the open.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A11-F1** Declare AI use on every post | Say on every post whether AI was used and how, with your usual answer remembered. | core | P1 AI-use field; P2 Declaration step; P3 Our AI tools mark what they make; P4 Signing certificate |
| **SOC-A11-F2** See Human, AI or both | Switch each feed to show the Human section, the AI section or both, and change it any time. | core | P1 Section filter; P2 Section switch |
| **SOC-A11-F3** Labelled bots and AI agents | Know at a glance when an account is a bot or AI agent, and who is responsible for it. | core | P1 Agent account type; P2 Section follows the answer, not the route |
| **SOC-A11-F4** Checked, not assumed | See content credentials when a photo or video carries them, while moderators get detection signals as hints, never verdicts. | next | P1 Content credentials reader; P2 Keep credentials, drop location; P3 Detection signals research |
| **SOC-A11-F5** Fix wrong sections in the open | Flag a post in the wrong section; a moderator moves it with the reason logged in public, and the creator can appeal. | core | P1 Wrong-section report; P2 Section move and appeal |
| **SOC-A11-F6** Mark your work as human-made | Attach content credentials to photos and videos made in the Spare Social camera or Spare Studio. | later | P1 Sign your own media |
| **SOC-A11-F7** Sections everywhere, across the network | See the Human and AI sections work for posts from other networks and in comments, replies, messages and live chat, and have your own answers travel with your posts. | core | P1 Posts from other networks; P2 Comments, replies, messages and live chat; P3 Answers and labels sent over the network |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A11-F1-P1 | **AI-use field**: None, AI-assisted or AI-made, with an optional note on which tools were used. | data | small | SOC-A3-F1-P1 | [M09](STATUS.md) |
| SOC-A11-F1-P2 | **Declaration step**: The composer asks every time, with the last answer pre-filled, never skipped. | interface | small | SOC-A11-F1-P1 | [M09](STATUS.md) |
| SOC-A11-F1-P3 | **Our AI tools mark what they make**: Background replacement, effects, AI dubbing and alt text suggestions add machine-readable C2PA content credentials to what they make and pre-fill the AI-use answer, as the EU AI Act's article 50 marking duties ask (start dates to be checked). | library | medium | engine-media, SOC-A11-F1-P1, SOC-A11-F1-P4 | [M09](STATUS.md) |
| SOC-A11-F1-P4 | **Signing certificate**: A certificate for signing content credentials from a provider on the C2PA trust list, so other apps accept Spareware's signatures. | legal | small |  | [M09](STATUS.md) |
| SOC-A11-F2-P1 | **Section filter**: Every feed, search and profile filters by section on the server. | service | small | SOC-A11-F1-P1, SOC-A10-F1-P1 | [M09](STATUS.md) |
| SOC-A11-F2-P2 | **Section switch**: A clear Human, AI, Both switch at the top of every feed. | interface | small | SOC-A11-F2-P1 |  |
| SOC-A11-F3-P1 | **Agent account type**: Bot and agent accounts carry a label and the human or organisation that owns them. | data | small | svc-accounts, SOC-A1-F2-P1 | [M09](STATUS.md) |
| SOC-A11-F3-P2 | **Section follows the answer, not the route**: A post goes to the section its AI-use answer gives, whether it came from the app, the command API, MCP, scheduling, bulk upload, Spare Studio or another client; bot and agent accounts' own posts go to the AI section, and a post an agent sends for a person carries an agent label without changing its section. | service | small | SOC-A11-F1-P1, SOC-A11-F3-P1, SOC-A23-F1-P1, kit-mcp | [M09](STATUS.md) |
| SOC-A11-F4-P1 | **Content credentials reader**: Read and show C2PA content credentials using an openly licensed library. | library | medium | engine-media | [M09](STATUS.md) |
| SOC-A11-F4-P2 | **Keep credentials, drop location**: Keep content credentials on upload while removing hidden location: the location is redacted inside the credentials and the result re-signed, so the credential stays valid. | library | small | SOC-A17-F1-P3, SOC-A11-F4-P1, SOC-A11-F1-P4 | [M09](STATUS.md) |
| SOC-A11-F4-P3 | **Detection signals research**: Compare open AI-detection methods on accuracy, bias and licence, for use as moderator hints only. | research | medium |  | [M09](STATUS.md) |
| SOC-A11-F5-P1 | **Wrong-section report**: A report type for posts in the wrong section. | data | small | SOC-A13-F1-P1 | [M09](STATUS.md) |
| SOC-A11-F5-P2 | **Section move and appeal**: Moderators move a post with a logged reason; the creator can appeal. | service | small | SOC-A13-F5-P1, SOC-A13-F6-P1 | [M09](STATUS.md) |
| SOC-A11-F6-P1 | **Sign your own media**: Add C2PA content credentials when media is captured or exported through Spareware tools, signed with the co-op's certificate (SOC-A11-F1-P4). | library | medium | engine-media, SOC-A11-F4-P1, SOC-A11-F1-P3, SOC-A11-F1-P4 | [M09](STATUS.md) |
| SOC-A11-F7-P1 | **Posts from other networks**: Posts from other servers keep the AI-use answer their protocol sends; posts with none are marked as not declared, and each person chooses whether undeclared posts show in their Human section. | service | small | SOC-A11-F2-P1, SOC-A18-F4-P1 | [M09](STATUS.md) |
| SOC-A11-F7-P2 | **Comments, replies, messages and live chat**: Comments, replies and live chat carry the same AI-use answers and agent labels as posts, so a bot's reply never shows in the Human view of a thread, and messages from an agent show its label. | service | small | SOC-A11-F2-P1, SOC-A3-F4-P2, SOC-A11-F3-P1 | [M09](STATUS.md) |
| SOC-A11-F7-P3 | **Answers and labels sent over the network**: AI-use answers and agent labels go out with every post: as AT Protocol self-labels, and as a published ActivityPub extension other servers can read. | library | medium | SOC-A11-F1-P1, SOC-A11-F3-P1, SOC-A18-F1-P5 | [M01](STATUS.md) |

## SOC-A12 Personal controls and filters

What each person decides for themselves: mute, snooze, block and restrict, hidden words, content labels and how to show them, community filters to subscribe to, who can reach, quote or follow you (bots and agents included), and help when a pile-on starts.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A12-F1** Mute, block and restrict | Mute someone you don't want to see, block someone who shouldn't reach you, or restrict them so their comments need your approval. | core | P1 Mute, block and restrict; P2 Shared block lists; P3 Blocks that cover every profile; P4 Snooze for 30 days |
| **SOC-A12-F2** Hide words and topics | Hide posts, comments and message requests with words or tags you choose, for a while or for good. | core | P1 Hidden words |
| **SOC-A12-F3** Content labels your way | For each label, such as nudity, violence or spoilers, choose to show, blur or hide. | core | P1 Label set and choices; P2 Blur and reveal |
| **SOC-A12-F4** Subscribe to community filters | Subscribe to labelling services run by groups you trust, and see why something was labelled. | next | P1 Labelling services; P2 Labeller directory |
| **SOC-A12-F5** Choose who can reach you | Decide who can comment, message, mention, tag, remix or find you by phone or email. | core | P1 Interaction settings; P2 Pile-on limits; P3 Quote and repost controls; P4 Keep bots and agents away |
| **SOC-A12-F6** Help when you are harassed | Clear abusive comments in bulk, keep evidence, and get help from an online safety agency, such as Netsafe in New Zealand, or the police. | core | P1 Bulk comment tools; P2 Evidence export |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A12-F1-P1 | **Mute, block and restrict**: Three levels of distance, enforced across feeds, comments, messages, search and tags. | service | medium | SOC-A2-F1-P1 | [M04](STATUS.md) |
| SOC-A12-F1-P2 | **Shared block lists**: Publish a block or mute list, and subscribe to lists others publish. | data | small | SOC-A12-F1-P1 | [M04](STATUS.md) |
| SOC-A12-F1-P3 | **Blocks that cover every profile**: A block covers every profile on the blocked person's login (SOC-A1-F4) and new accounts they are likely to make, found by published signals, without showing anyone that the profiles are linked. | service | small | SOC-A12-F1-P1, KIT-A1-F1-P1 | [M04](STATUS.md) |
| SOC-A12-F1-P4 | **Snooze for 30 days**: Snooze a person, page or group for 30 days, after which they come back by themselves. | data | small | SOC-A12-F1-P1 |  |
| SOC-A12-F2-P1 | **Hidden words**: Muted words, phrases and tags with an optional end date, matched across languages' spelling forms; the one hidden-words library for posts, comments, live chat, groups and message requests, and a candidate kit library, since other apps filter text too. | library | small | engine-text |  |
| SOC-A12-F3-P1 | **Label set and choices**: Spare Social's published label set on the kit's content labels and personal filters (KIT-A10-F13-P1, KIT-A10-F13-P2), with each viewer's show, blur or hide choice per label. | integration | small | KIT-A10-F13-P1, KIT-A10-F13-P2 | [M04](STATUS.md) |
| SOC-A12-F3-P2 | **Blur and reveal**: Blurred previews with the label shown and a tap to reveal. | interface | small | SOC-A12-F3-P1 |  |
| SOC-A12-F4-P1 | **Labelling services**: A format for independent labellers to publish labels, and for members to subscribe; a candidate kit part, since labels from outside services could serve every app. | service | medium | SOC-A12-F3-P1, SOC-A18-F1-P1 | [M04](STATUS.md) |
| SOC-A12-F4-P2 | **Labeller directory**: Find labellers, read their rules and see who runs them. | interface | small | SOC-A12-F4-P1 |  |
| SOC-A12-F5-P1 | **Interaction settings**: One page for comment, message, mention, tag, quote, agent and discovery settings; the remix switches join it when remixing arrives (SOC-A5-F5-P1). | interface | small | SOC-A3-F4-P3 |  |
| SOC-A12-F5-P2 | **Pile-on limits**: Turn on a temporary limit so only long-standing followers can reply or message. | service | small | SOC-A12-F5-P1 |  |
| SOC-A12-F5-P3 | **Quote and repost controls**: Turn off quotes or reposts of a post, and detach your post from someone else's quote of it. | service | small | SOC-A3-F5-P1, SOC-A12-F5-P1 |  |
| SOC-A12-F5-P4 | **Keep bots and agents away**: A switch for whether bots and AI agents may reply to, mention, message or follow you, enforced on the command API and MCP as well as in the app. | service | small | SOC-A11-F3-P1, SOC-A12-F5-P1, SOC-A23-F1-P1 |  |
| SOC-A12-F6-P1 | **Bulk comment tools**: Select and hide or delete many comments, and block their writers in one go. | interface | small | SOC-A3-F4-P3 |  |
| SOC-A12-F6-P2 | **Evidence export**: Save a dated record of abusive posts and messages to give to an online safety agency, such as Netsafe in New Zealand, or the police. | service | small | SOC-A19-F1-P1 | [M02](STATUS.md) |

## SOC-A13 Moderation, legal duties and transparency

Removing only what the law requires, by published rules, in the open: reports from anyone, the rule book, fast removal of illegal material including intimate images and court-ordered content, legal deadlines met around the clock, copyright and trade mark notices, a public removal log, statements of reasons and appeals, a published ladder before any ban, vetted and protected moderators members elect, spam and fake-account defences, and police and government requests.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A13-F1** Report anything | Report a post, comment, profile, message, group, event or live stream against a named rule, and hear what happened. | core | P1 Report flow; P2 Reports as operations; P3 Reporter updates; P4 Report without an account |
| **SOC-A13-F2** Read the rules | Read the rules in plain words, each linked to the law that requires it, with every change voted on and dated. | core | P1 Rule book; P2 Rules mapped to the law; P3 Election period rules; P4 Adult sexual content decision; P5 Platform integrity rules |
| **SOC-A13-F3** Illegal material removed fast | Know that illegal material, such as child abuse and terrorist material, intimate images shared without consent, credible threats, scams and content a court orders down, is found and removed fast. | core | P1 Known-material matching; P2 Legal duties in each country; P3 Overseas legal duties; P4 Urgent escalation; P5 Country-only restriction; P6 Court orders; P7 Whose laws count; P8 Legal deadline clock and on-call cover; P9 Scams and fraud taken down; P10 Known video and crisis protocol; P11 EU and UK platform duties |
| **SOC-A13-F4** Copyright notices | Send a copyright notice, answer one with a counter-notice, and see the outcome. | core | P1 Notice and counter-notice; P2 Rights-holder matching research; P3 Repeat infringer policy; P4 Trade mark complaints |
| **SOC-A13-F5** See every removal in public | Browse a public log of every removal, ban, label and section move, with the rule broken, and read regular reports. | core | P1 Public removal log; P2 Transparency reports |
| **SOC-A13-F6** Appeal any decision | Appeal a removal, ban, label or move, get a different moderator, then a member panel. | core | P1 Appeals; P2 Statement of reasons |
| **SOC-A13-F7** Moderators members elect | Vote for moderators with fixed terms, and see what tools and limits they work under. | core | P1 Moderator elections; P2 Moderator workbench; P3 Moderator wellbeing; P4 Moderator vetting and training; P5 Handling objectionable material lawfully; P6 Moderators' status and safety duties; P7 Moderators protected from retaliation |
| **SOC-A13-F8** Stop spam, bots and fake accounts | Keep feeds and comments free of spam and fake networks, by published methods. | core | P1 New-account limits; P2 Spam detection; P3 Fake network research |
| **SOC-A13-F9** Police and government requests | Know that any request for your data is checked against the law, told to you where allowed, and counted in public. | core | P1 Request desk |
| **SOC-A13-F10** Get intimate images taken down fast | Have an intimate image or sexual deepfake of you removed, with known copies, within 48 hours, even without an account, and stopped from coming back. | core | P1 Intimate image report form; P2 Removal within 48 hours; P3 Re-upload blocking |
| **SOC-A13-F11** Know the steps before a ban | See a published ladder of warning, time-limited suspension and ban for repeated legal breaches, applied the same way to everyone, with expiry and a way back. | core | P1 Enforcement ladder; P2 Strikes and restoring |
| **SOC-A13-F12** Ask for your image to come down | Ask for a photo or video that shows you, your child or your home to be blurred or removed, even if you are not a member. | core | P1 Privacy complaints about people shown |
| **SOC-A13-F13** Safety bodies heard fast, decisions checked | Know reports from recognised safety bodies such as Netsafe and DIA in New Zealand and eSafety in Australia are reviewed first, and that a sample of decisions gets a second look. | next | P1 Trusted flaggers; P2 Second review and conflict checks |
| **SOC-A13-F14** Moderation in every language | Know posts in every language members use are moderated, with machine translation for moderators and the original always shown. | next | P1 Moderators for each language; P2 Translation for moderators |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A13-F1-P1 | **Report flow**: A short flow on every item, built on the kit's report forms (KIT-A10-F3-P2): pick the rule, add detail, choose to block too. | interface | medium | svc-trust, KIT-A10-F3-P2 | [M04](STATUS.md) |
| SOC-A13-F1-P2 | **Reports as operations**: Every report and decision is an operation in the kit's reports service (KIT-A10-F3-P1), so the public log and the audit trail build themselves. | data | small | kit-ops, KIT-A10-F3-P1 | [M04](STATUS.md) |
| SOC-A13-F1-P3 | **Reporter updates**: Tell the reporter what was decided and why, and how to appeal. | service | small | svc-messaging, SOC-A13-F1-P2 |  |
| SOC-A13-F1-P4 | **Report without an account**: Public web forms on the kit's reports without an account (KIT-A10-F3-P3) for legal notices, privacy complaints, intimate images, defamation, copyright and police, since victims, rights holders and officials often are not members. | integration | small | KIT-A10-F3-P3, SOC-A13-F1-P2 | [M02](STATUS.md) |
| SOC-A13-F2-P1 | **Rule book**: Versioned plain-language rules on the kit's rules library (KIT-A10-F5-P1), changed only by member vote, with the history public; the promise to remove only what the law requires is a protected rule (SOC-A22-F1-P3). | data | small | svc-voting, KIT-A10-F5-P1, SOC-A22-F1-P3 | [M04](STATUS.md) |
| SOC-A13-F2-P2 | **Rules mapped to the law**: For each rule, the law in each member's country that requires it, starting with New Zealand's. | research | medium | SOC-A13-F2-P1 | [M02](STATUS.md) |
| SOC-A13-F2-P3 | **Election period rules**: Each country's election advertising rules, starting with New Zealand's, and what they mean for posts during an election period. | research | small |  | [M02](STATUS.md) |
| SOC-A13-F2-P4 | **Adult sexual content decision**: Decide whether lawful adult sexual content is allowed with age checks for viewers and age and consent checks for everyone shown, allowed only outside the app-store apps, or ruled out by a published rule, given UK and Australian age rules, US state age laws, card network and payment provider rules and app store rules; blur labels alone meet none of these. | research | medium |  | [M02](STATUS.md) |
| SOC-A13-F2-P5 | **Platform integrity rules**: Published rules for removals that keep the platform working but are not about illegal content: spam, fake accounts, ban evasion, impersonation, malware and phishing links, and automated floods, each removal naming its rule like any other. | legal | small | SOC-A13-F2-P1 | [M04](STATUS.md) |
| SOC-A13-F3-P1 | **Known-material matching**: Checks uploads with the kit's known-content matching (KIT-A10-F10-P2) against hash lists from authorised bodies, such as PDQ for images; video hashes come with SOC-A13-F3-P10. | integration | medium | KIT-A10-F10-P2, SOC-A17-F1-P2 | [M07](STATUS.md) |
| SOC-A13-F3-P2 | **Legal duties in each country**: What each country requires and how fast, starting with New Zealand: objectionable material and DIA take-down notices, the Harmful Digital Communications Act 2015 (intimate images, and its safe harbour of passing a complaint to the author within 48 hours), court orders, fraud and scams, illegal sales such as arms and drugs, incitement under the Human Rights Act 1993, privacy, defamation and copyright. | research | large |  | [M02](STATUS.md) |
| SOC-A13-F3-P3 | **Overseas legal duties**: Duties in other countries, such as the EU Digital Services Act, the UK Online Safety Act and Australia's Online Safety Act. | research | large |  | [M02](STATUS.md) |
| SOC-A13-F3-P4 | **Urgent escalation**: A fast lane to police and the Department of Internal Affairs for child safety and threats to life, with evidence kept as the law requires. | service | medium | SOC-A17-F6-P2 | [M07](STATUS.md) |
| SOC-A13-F3-P5 | **Country-only restriction**: Hide a post only in a country whose law requires it, logged in public, instead of removing it everywhere, and only where the demand passes the published test (SOC-A13-F3-P7). | service | medium | SOC-A3-F2-P1, SOC-A13-F3-P7 | [M02](STATUS.md) |
| SOC-A13-F3-P6 | **Court orders**: Court orders handled fast and limited to the country where the order applies, each logged without repeating the suppressed detail, such as New Zealand's name suppression breaches, take-down orders under its 2019 contempt of court law and Harmful Digital Communications Act orders. | service | small | SOC-A13-F3-P5, KIT-A10-F9-P2 | [M02](STATUS.md) |
| SOC-A13-F3-P7 | **Whose laws count**: A published policy on which governments' removal demands are obeyed, tested against human rights principles such as the Global Network Initiative's, with every demand and its outcome logged in public. | legal | small | SOC-A13-F2-P1 | [M02](STATUS.md) |
| SOC-A13-F3-P8 | **Legal deadline clock and on-call cover**: A clock for each kind of notice (New Zealand's Harmful Digital Communications Act 48 hours, Australia's eSafety 24 hours, US TAKE IT DOWN Act 48 hours, New Zealand's DIA take-down notices and live terror content as soon as reasonably practicable), an on-call rota across time zones, and alerts before any deadline is missed. | service | medium | KIT-A10-F9-P2, KIT-A21-F3-P1, SOC-A13-F1-P2 |  |
| SOC-A13-F3-P9 | **Scams and fraud taken down**: Fraud and scams are illegal and removed under the same rules, with a fast path for scams that use deepfakes of public figures or impersonate brands. | service | small | SOC-A13-F1-P2, SOC-A13-F8-P2 | [M07](STATUS.md) |
| SOC-A13-F3-P10 | **Known video and crisis protocol**: Video hashes for known terror and abuse video (vPDQ or TMK+PDQF, released openly by Meta, licence checked before use), GIFCT hash sharing, and the Christchurch Call crisis protocol for live attacks. | integration | medium | SOC-A13-F3-P1, SOC-A17-F2-P1 | [M07](STATUS.md) |
| SOC-A13-F3-P11 | **EU and UK platform duties**: An EU legal representative and contact points, a notice form and statements of reasons under the EU Digital Services Act, its extra platform duties once the co-op is no longer a small enterprise, and the UK Online Safety Act illegal-content risk assessment, built with the kit's overseas platform duties (KIT-A18-F13-P1); open protocols reach EU and UK members from day one. | legal | medium | SOC-A13-F3-P3, KIT-A18-F13-P1 | [M02](STATUS.md) |
| SOC-A13-F4-P1 | **Notice and counter-notice**: Notice and counter-notice forms for posts and videos, tracked on the kit's legal notice deadlines (KIT-A10-F9-P2) and sharing forms with the kit's listing notices (KIT-A10-F21-P2), fitting each country's notice rules, such as New Zealand's Copyright Act 1994 and the US notice process. | integration | medium | KIT-A10-F9-P2, SOC-A13-F1-P2 | [M02](STATUS.md) |
| SOC-A13-F4-P2 | **Rights-holder matching research**: How open audio and video fingerprinting could flag likely matches for human review, never automatic strikes. | research | large |  | [M07](STATUS.md) |
| SOC-A13-F4-P3 | **Repeat infringer policy**: A published policy for accounts with repeated upheld copyright notices. | legal | small | SOC-A13-F2-P1 | [M02](STATUS.md) |
| SOC-A13-F4-P4 | **Trade mark complaints**: Trade mark and brand complaints about handles, pages and channels, with a counter-notice and the outcome logged; the kit's trade mark notices cover listings only (KIT-A10-F21). | service | small | SOC-A13-F4-P1, SOC-A1-F1-P3 | [M02](STATUS.md) |
| SOC-A13-F5-P1 | **Public removal log**: Spare Social's actions in the kit's public removal log (KIT-A10-F7-P1): every removal, ban, label and section move with its rule, searchable, never exposing victims or illegal material, with moderators shown by pseudonymous IDs (SOC-A13-F7-P7). | integration | medium | KIT-A10-F7-P1, SOC-A13-F1-P2, SOC-A13-F7-P7 | [M04](STATUS.md) |
| SOC-A13-F5-P2 | **Transparency reports**: Regular counts per rule, appeal outcomes and government requests as open data, built by the kit's transparency reports (KIT-A10-F15-P1). | integration | small | KIT-A10-F15-P1, SOC-A13-F5-P1 | [M04](STATUS.md) |
| SOC-A13-F6-P1 | **Appeals**: Two-step appeals with deadlines: a fresh moderator, then a member panel drawn at random by the kit (KIT-A11-F4-P1, KIT-A11-F5-P1), with the outcome logged. | integration | medium | KIT-A11-F4-P1, KIT-A11-F5-P1, SOC-A13-F1-P2 | [M04](STATUS.md) |
| SOC-A13-F6-P2 | **Statement of reasons**: The person whose post, account or group was acted on is told the rule, the reason and how to appeal, through the kit's statements of reasons (KIT-A10-F8-P1). | integration | small | KIT-A10-F8-P1, SOC-A13-F1-P2 | [M04](STATUS.md) |
| SOC-A13-F7-P1 | **Moderator elections**: Elections, terms and recall through the kit's elections (KIT-A12-F6-P1), with one vote per person on the member register, never per profile, pseudonymous account or agent. | integration | medium | svc-voting, KIT-A12-F6-P1, KIT-A14-F2-P1 | [M04](STATUS.md) |
| SOC-A13-F7-P2 | **Moderator workbench**: Spare Social's queues, context, past decisions and rule picker on the kit's moderator tools (KIT-A10-F6-P3), with every action logged. | integration | medium | KIT-A10-F6-P3, SOC-A13-F1-P2 | [M04](STATUS.md) |
| SOC-A13-F7-P3 | **Moderator wellbeing**: Blurred previews, exposure limits, rotation and paid support for people who review harmful material. | research | small |  | [M04](STATUS.md) |
| SOC-A13-F7-P4 | **Moderator vetting and training**: Checks before anyone reviews the worst material, using the kit's background checks (KIT-A2-F9-P2) where the law allows, and training and certificates from the kit's training module (KIT-A25-F2-P1). | integration | medium | KIT-A2-F9-P2, KIT-A25-F2-P1 | [M04](STATUS.md) |
| SOC-A13-F7-P5 | **Handling objectionable material lawfully**: A legal basis for moderators handling objectionable material, which is an offence to possess in many countries: hash matches go straight to the country's authority (example: New Zealand's DIA), and a person sees such material only when they must, blurred first, with every view logged. | legal | medium | SOC-A13-F3-P1, SOC-A13-F3-P4 | [M02](STATUS.md) |
| SOC-A13-F7-P6 | **Moderators' status and safety duties**: Whether paid moderators are employees or contractors, how their pay is taxed, and the co-op's health and safety duties to every moderator, volunteers included, under each country's workplace safety law (example: New Zealand's Health and Safety at Work Act 2015). | legal | medium | KIT-A23-F1-P1, KIT-A24-F1-P1, KIT-A11-F4-P4 | [M02](STATUS.md) |
| SOC-A13-F7-P7 | **Moderators protected from retaliation**: Pseudonymous moderator IDs in the public log and in appeals, so decisions stay open without exposing who made each one, and safety support for elected moderators who become public figures. | service | small | SOC-A13-F7-P2 | [M04](STATUS.md) |
| SOC-A13-F8-P1 | **New-account limits**: New-account limits on the kit's spam limits (KIT-A10-F11-P1), set by Spare Social's rules and easing as an account earns trust. | integration | small | KIT-A10-F11-P1 | [M07](STATUS.md) |
| SOC-A13-F8-P2 | **Spam detection**: Spam detection on published signals such as repeated links and copy-paste floods. | service | large | svc-trust | [M07](STATUS.md) |
| SOC-A13-F8-P3 | **Fake network research**: Proven ways to find coordinated fake accounts, and what can be published without helping spammers. | research | medium |  | [M07](STATUS.md) |
| SOC-A13-F9-P1 | **Request desk**: Uses the kit's published process for police and court requests (KIT-A10-F9-P3), tracked on its deadlines (KIT-A10-F9-P2): intake, legal check, member notice where allowed, and counts for the transparency report. | integration | small | KIT-A10-F9-P3, KIT-A10-F9-P2, SOC-A13-F5-P2 | [M02](STATUS.md) |
| SOC-A13-F10-P1 | **Intimate image report form**: A clearly shown public form, usable without an account, for intimate images shared without consent and sexual deepfakes, answered by the kit's trained responders (KIT-A10-F18-P2). | interface | small | SOC-A13-F1-P4, KIT-A10-F18-P2 | [M02](STATUS.md) |
| SOC-A13-F10-P2 | **Removal within 48 hours**: Removes the image and known copies within 48 hours, as the US TAKE IT DOWN Act requires since 19 May 2026, and faster where New Zealand's Harmful Digital Communications Act or Australia's eSafety Commissioner asks. | service | medium | SOC-A13-F10-P1, SOC-A13-F3-P8 | [M02](STATUS.md) |
| SOC-A13-F10-P3 | **Re-upload blocking**: Hashes of removed images block re-uploads, and people can add hashes made on their own device, in the way StopNCII works, without ever sending the image. | library | medium | SOC-A13-F3-P1, SOC-A13-F10-P2 | [M07](STATUS.md) |
| SOC-A13-F11-P1 | **Enforcement ladder**: Published steps for repeated breaches such as threats or sharing intimate images: a warning, a suspension for a set time, a ban, when each step expires, and how an account is restored. | legal | small | SOC-A13-F2-P1 | [M04](STATUS.md) |
| SOC-A13-F11-P2 | **Strikes and restoring**: Records each upheld breach as a strike that expires on time, applies the next step on the ladder, and restores the account when a ban ends or an appeal wins. | service | small | SOC-A13-F11-P1, SOC-A13-F1-P2 |  |
| SOC-A13-F12-P1 | **Privacy complaints about people shown**: A complaint path for bystanders, children and anyone else shown in someone else's photo or video, tested against each country's privacy and online harm law (example: New Zealand's Privacy Act 2020 and Harmful Digital Communications Act principles), with blurring offered before removal where that is enough. | service | small | SOC-A13-F1-P4, SOC-A13-F1-P2 | [M02](STATUS.md) |
| SOC-A13-F13-P1 | **Trusted flaggers**: A priority queue for recognised safety bodies, such as Netsafe (New Zealand's approved agency under the Harmful Digital Communications Act), New Zealand's DIA and Australia's eSafety, on the kit's trusted flaggers (KIT-A10-F17-P1), which must move up to next for this. | integration | small | KIT-A10-F17-P1, SOC-A13-F1-P2 |  |
| SOC-A13-F13-P2 | **Second review and conflict checks**: A random sample of moderator decisions gets a second review, agreement rates are published, and moderators declare conflicts of interest, using the kit's decision sampling (KIT-A10-F20-P1). | integration | small | KIT-A10-F20-P1, SOC-A13-F7-P2 |  |
| SOC-A13-F14-P1 | **Moderators for each language**: Recruits and elects moderators for each language members post in, with a published list of the languages covered and the gaps. | service | small | SOC-A13-F7-P1 | [M04](STATUS.md) |
| SOC-A13-F14-P2 | **Translation for moderators**: Machine translation in the moderator tools with the original always shown, using the kit's translation service (KIT-A19-F10-P2). | integration | small | KIT-A19-F10-P2, SOC-A13-F7-P2 |  |

## SOC-A14 Young people and wellbeing

Minimum-age rules for each country, safe defaults for teens, protection from grooming, sextortion and scams, child safety risk assessments, family supervision that teens can see, money rules for young people, time tools for everyone, and help for people in crisis.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A14-F1** Age rules for each country | Join only if you meet your country's minimum age, checked in the way the law there requires. | core | P1 Age rules engine; P2 Age assurance; P3 Minimum-age law research; P4 Parent consent where the law asks; P5 Biometric code for faces |
| **SOC-A14-F2** Teen accounts with safe defaults | Teens start private, get messages only from people they follow, see sensitive labels hidden, and are not suggested to adults. | core | P1 Teen defaults; P2 Adult contact limits |
| **SOC-A14-F3** Family supervision | A parent and teen link accounts; the parent sees time spent and who the teen follows, never message contents. | next | P1 Linked family accounts; P2 Family dashboard |
| **SOC-A14-F4** Time tools for everyone | Set a daily limit, quiet hours and break reminders, and hide like and view counts. | next | P1 Time counter on the device; P2 Limits, quiet hours and breaks; P3 Hide counts |
| **SOC-A14-F5** Help in a crisis | When someone searches for or posts about self-harm, they see local help such as 1737 in New Zealand, and anyone can flag a person at risk. | core | P1 Help resources per country; P2 Person-at-risk reports |
| **SOC-A14-F6** A space for children | Give children under the minimum age a separate space of chosen videos, run by their parents. | later | P1 Children's space research; P2 Children's space |
| **SOC-A14-F7** Protection from grooming and sextortion | As a teen, get a warning when a new or unconnected adult asks for images or money, and fast help if someone threatens you with images. | core | P1 Risky request warnings; P2 Sextortion help |
| **SOC-A14-F8** Scam warnings | See a warning when a message or post looks like a scam, such as an investment pitch using a deepfake of a public figure. | next | P1 Scam warnings in chats and posts |
| **SOC-A14-F9** Children's safety checked by design | Know Spare Social has assessed its risks to children and meets children's design codes before features ship. | core | P1 Children's risk assessment; P2 Children's design codes |
| **SOC-A14-F10** Money rules for young people | As a teen, tip or join memberships only within limits a parent approves, and get paid as a young creator through a guardian. | next | P1 Teen spending limits; P2 Young creators' payouts; P3 Children in paid content |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A14-F1-P1 | **Age rules engine**: Minimum ages, teen settings and consent ages per country, applied at sign-up and when the law changes; a candidate kit service beside the kit's minimum-age rule (KIT-A1-F1-P5) and age checks (KIT-A2-F8), since Spare Market, Spare Ride and others need the same rules. | library | medium | KIT-A1-F1-P5 | [M02](STATUS.md) |
| SOC-A14-F1-P2 | **Age assurance**: Age checks through the kit's age assurance (KIT-A2-F8-P1), returning only whether a person is over the age set and meeting the biometric code (SOC-A14-F1-P5). | integration | medium | KIT-A2-F8-P1, SOC-A14-F1-P5 | [M07](STATUS.md) |
| SOC-A14-F1-P3 | **Minimum-age law research**: Australia's under-16 law, New Zealand's minimum age bill introduced in August 2026 (passage unverified), and UK and EU child rules. | research | medium |  | [M02](STATUS.md) |
| SOC-A14-F1-P4 | **Parent consent where the law asks**: Gets a parent's consent where a country's law asks for it before a young person's data is used, such as the EU's GDPR article 8 and the US children's privacy rule (COPPA). | service | small | SOC-A14-F1-P1 | [M02](STATUS.md) |
| SOC-A14-F1-P5 | **Biometric code for faces**: Facial age estimation, face blurring and any face or person detection meet each country's biometric rules, such as New Zealand's Biometric Processing Privacy Code (issued 2025, amended 2026; start dates to be checked), building on the kit's compliance work (KIT-A2-F1-P4). | legal | small | KIT-A2-F1-P4 | [M02](STATUS.md) |
| SOC-A14-F2-P1 | **Teen defaults**: Private account, limited messages, hidden sensitive labels and no live streaming until an age set by the rules. | service | small | SOC-A14-F1-P1, SOC-A12-F3-P1 | [M07](STATUS.md) |
| SOC-A14-F2-P2 | **Adult contact limits**: Adults cannot find, suggest or message teens they are not connected to; search, suggestions (SOC-A2-F4) and message requests all apply this limit. | service | medium | SOC-A2-F1-P1, SOC-A9-F1-P4, SOC-A14-F1-P1 | [M07](STATUS.md) |
| SOC-A14-F3-P1 | **Linked family accounts**: Linking that needs both to agree, and that the teen can always see. | service | medium | svc-accounts |  |
| SOC-A14-F3-P2 | **Family dashboard**: Time spent, limits, follows and blocks, in one place for the parent. | interface | medium | SOC-A14-F3-P1 |  |
| SOC-A14-F4-P1 | **Time counter on the device**: Time spent counted on the device only, never sent anywhere. | library | small |  |  |
| SOC-A14-F4-P2 | **Limits, quiet hours and breaks**: Daily limits, quiet hours that hold notifications, and break reminders. | interface | small | SOC-A20-F3-P1 |  |
| SOC-A14-F4-P3 | **Hide counts**: Hide like and view counts on your own posts, on everyone's, or both. | data | small | SOC-A3-F4-P1 |  |
| SOC-A14-F5-P1 | **Help resources per country**: Uses the kit's helplines for each country (KIT-A10-F1-P3), such as 1737 in New Zealand, kept current. | integration | small | KIT-A10-F1-P3 |  |
| SOC-A14-F5-P2 | **Person-at-risk reports**: A report path reviewed by a person fast, with help sent to the person at risk. | service | small | svc-trust, SOC-A13-F1-P1 | [M07](STATUS.md) |
| SOC-A14-F6-P1 | **Children's space research**: What the law allows for under-13s in each country, starting with New Zealand, and whether the co-op should offer it at all. | research | medium |  | [M02](STATUS.md) |
| SOC-A14-F6-P2 | **Children's space**: Channels approved by parents, with no comments, no messages and no discovery. | service | large | SOC-A14-F3-P1 |  |
| SOC-A14-F7-P1 | **Risky request warnings**: On the device, warns a teen when a new or unconnected adult asks for images, money or a move to another app, without sending the messages anywhere. | library | medium | SOC-A14-F2-P1, SOC-A9-F7-P2 | [M07](STATUS.md) |
| SOC-A14-F7-P2 | **Sextortion help**: A fast path for a teen threatened with images: block, report, removal through the intimate image path (SOC-A13-F10) and local help, with plain advice not to pay. | service | small | SOC-A13-F10-P1, SOC-A14-F5-P1 | [M07](STATUS.md) |
| SOC-A14-F8-P1 | **Scam warnings in chats and posts**: Uses the kit's scam warnings (KIT-A8-F9-P1) in chats and on posts, run on the device in encrypted chats so no message leaves it. | integration | small | KIT-A8-F9-P1, SOC-A9-F1-P2 |  |
| SOC-A14-F9-P1 | **Children's risk assessment**: A child safety risk assessment for each feature, as the UK Online Safety Act requires and New Zealand's Online Safety (Minimum Age and Child Safety Risk Assessment) Bill proposes (introduced 24 August 2026; passage unverified), refreshed when features change. | legal | medium | SOC-A14-F1-P3 | [M02](STATUS.md) |
| SOC-A14-F9-P2 | **Children's design codes**: Meets the UK Age Appropriate Design Code and Australia's Children's Online Privacy Code (due to be registered by 10 December 2026), with each finding turned into a setting or a rule. | legal | small | SOC-A14-F9-P1 | [M02](STATUS.md) |
| SOC-A14-F10-P1 | **Teen spending limits**: Spending limits and parent approval for tips, memberships and purchases by teens. | service | small | SOC-A15-F1-P1, SOC-A14-F1-P1 |  |
| SOC-A14-F10-P2 | **Young creators' payouts**: Under-18 creators are paid out through a guardian's account, since payment providers require adult account holders. | integration | small | SOC-A15-F4-P1 |  |
| SOC-A14-F10-P3 | **Children in paid content**: Rules for children shown in paid or sponsored content, including setting aside their share of earnings where laws such as those in Illinois, California and Minnesota ask. | legal | small |  | [M02](STATUS.md) |

## SOC-A15 Creator support and money

Members pay creators directly: tips, monthly memberships with perks, one-off purchases, payouts, tax, honest sponsorship labels, shop links and refunds, sold in ways the app stores allow and protected from fraud. Creators keep everything except the payment fees, and there are no adverts.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A15-F1** Tip a creator | Send a creator a tip on a post, video or stream, all of which reaches them, and see what the card payment cost. | next | P1 Tips; P2 Tip button; P3 Card cost shown |
| **SOC-A15-F2** Memberships | Join a creator's monthly membership for members-only posts, videos, chats and badges. | next | P1 Recurring payments and tiers; P2 Members-only content; P3 Member badges and lists; P4 Fair membership terms |
| **SOC-A15-F3** Buy a video or series | Pay once to watch a video, course or series a creator sells. | later | P1 One-off purchases |
| **SOC-A15-F4** Get paid out | Set up payouts once, see your balance and statements, and get paid on a regular schedule. | next | P1 Payout set-up; P2 Earnings and statements |
| **SOC-A15-F5** Tax done right | Get the tax summaries you need, with sales tax handled where the platform must collect it (example: New Zealand GST). | next | P1 Tax research for creator payments; P2 Tax summaries; P3 Sales tax as the deemed supplier (example: New Zealand GST) |
| **SOC-A15-F6** Honest sponsorship labels | Mark a post as a paid partnership and tag the brand, so viewers always know. | core | P1 Paid partnership label; P2 Advertising rules research; P3 Promotion the law bans |
| **SOC-A15-F7** Link your shop | Tag products from your Spare Market shop or your own site in posts and on your profile. | later | P1 Product tags |
| **SOC-A15-F8** Refunds and payment disputes | Ask for a refund on a membership or purchase and have disputes handled fairly. | next | P1 Refunds and chargebacks |
| **SOC-A15-F9** No adverts, ever | Use Spare Social with no adverts and no data sold, now and later. | core | P1 No-advert rule |
| **SOC-A15-F10** Pay on any device | Pay for memberships, paid videos, tips and storage in a way each app store allows, so creators keep everything except payment fees on iPhone and Android too. | next | P1 App store payment rules; P2 Purchase plan per country |
| **SOC-A15-F11** Safe from payment fraud and laundering | Know tips and memberships are protected from stolen cards, card testing and money laundering. | next | P1 Tip and membership fraud checks; P2 Payout holds and negative balances; P3 Anti-money-laundering and sanctions |
| **SOC-A15-F12** Share earnings and give memberships | Split earnings with co-authors, channel teams and series collaborators, and give someone a membership as a gift. | later | P1 Revenue splits; P2 Gift memberships |
| **SOC-A15-F13** Sell to buyers overseas | Sell memberships and paid videos to buyers in other countries with their VAT or GST handled. | later | P1 Overseas VAT on digital sales |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A15-F1-P1 | **Tips**: Tips on the kit's tips (KIT-A3-F6-P1): the whole tip reaches the creator with no co-op fee, and the card cost is carried as a running cost on the open cost ledger, never taken from the tip, the same rule as every app. | integration | medium | KIT-A3-F6-P1 | [K02](../kit/STATUS.md) |
| SOC-A15-F1-P2 | **Tip button**: A tip button on posts, videos, profiles and live streams. | interface | small | SOC-A15-F1-P1 |  |
| SOC-A15-F1-P3 | **Card cost shown**: The card cost of each tip shown before paying, with where it is carried on the open cost ledger, under the kit's one rule for tips (KIT-A3-F6-P1). | interface | small | svc-ledger, KIT-A3-F4-P2, KIT-A3-F6-P1 | [K06](../kit/STATUS.md) |
| SOC-A15-F2-P1 | **Recurring payments and tiers**: Monthly tiers with prices set by the creator, on the kit's regular payments (KIT-A3-F12-P1). | integration | medium | KIT-A3-F12-P1 | [K02](../kit/STATUS.md) |
| SOC-A15-F2-P2 | **Members-only content**: Members become an audience for posts, videos, stories and chats, with signed media links. | service | small | SOC-A3-F2-P1, SOC-A17-F4-P4 |  |
| SOC-A15-F2-P3 | **Member badges and lists**: Badges for members and a member list for the creator. | interface | small | SOC-A15-F2-P1 |  |
| SOC-A15-F2-P4 | **Fair membership terms**: Cancel in one step, get notice before any price change, and get the refunds consumer law gives, including overseas auto-renewal rules. | service | small | KIT-A3-F12-P1, KIT-A18-F8-P1 |  |
| SOC-A15-F3-P1 | **One-off purchases**: Single purchases unlocking a video or series for the buyer, sold in the way each app store allows (SOC-A15-F10). | integration | medium | svc-payments, SOC-A5-F6-P1 |  |
| SOC-A15-F4-P1 | **Payout set-up**: Creator payout set-up on the kit's payouts (KIT-A3-F3-P1), including the identity checks the provider needs (KIT-A2-F1-P2). | integration | medium | KIT-A3-F3-P1, KIT-A2-F1-P2 | [K02](../kit/STATUS.md) |
| SOC-A15-F4-P2 | **Earnings and statements**: Balance, payouts and statements on the kit's earnings page (KIT-A3-F3-P3). | integration | small | KIT-A3-F3-P3, SOC-A15-F4-P1 |  |
| SOC-A15-F5-P1 | **Tax research for creator payments**: Which of the kit's tax rules apply to creator payments: sales tax on digital services (example: New Zealand GST), who is the seller of record (KIT-A3-F1-P5), income reporting to the tax office (example: New Zealand's IRD) and tax in other countries; needs a tax adviser's view. | research | medium | KIT-A4-F1-P1, KIT-A3-F1-P5 |  |
| SOC-A15-F5-P2 | **Tax summaries**: Yearly and on-demand summaries for creators from the kit's statements and tax office reporting (KIT-A4-F4-P1, KIT-A4-F3-P3). | integration | small | KIT-A4-F4-P1, KIT-A4-F3-P3 |  |
| SOC-A15-F5-P3 | **Sales tax as the deemed supplier (example: New Zealand GST)**: Where a country's sales tax law treats the co-op as the supplier of remote services sold through Spare Social, works out, collects, invoices and returns the tax. In New Zealand, the first country pack, that covers memberships and paid videos from offshore creators to New Zealand consumers (GST Act section 60C). Until tax in other countries is handled (SOC-A15-F13), paid content is sold only to buyers in countries whose pack handles it, starting with New Zealand. Needs a tax adviser's view. | service | medium | KIT-A4-F1-P2, KIT-A4-F9-P2, SOC-A15-F5-P1 |  |
| SOC-A15-F6-P1 | **Paid partnership label**: A label and brand tag on posts, searchable and shown in every feed. | data | small | SOC-A3-F1-P1 |  |
| SOC-A15-F6-P2 | **Advertising rules research**: Advertising standards rules for influencers in each country, starting with New Zealand's. | research | small |  | [M02](STATUS.md) |
| SOC-A15-F6-P3 | **Promotion the law bans**: Sponsored posts for things a country's law bans promoting are not allowed even with a label, such as vaping and tobacco products, offshore gambling and some medicines in New Zealand, and election posts carry the promoter statement the law asks for (example: New Zealand's Electoral Act). | legal | small | SOC-A15-F6-P2 | [M02](STATUS.md) |
| SOC-A15-F7-P1 | **Product tags**: Product tags that open the Spare Market listing, drawn from the kit's listing model (KIT-A5-F1-P1); checkout stays in Spare Market. | integration | small | KIT-A5-F1-P1, kit-market |  |
| SOC-A15-F8-P1 | **Refunds and chargebacks**: Refunds, card disputes and who carries a loss on the kit's refunds, disputes and loss rules (KIT-A3-F5-P1, KIT-A3-F5-P2, KIT-A3-F16-P1), with member-panel disputes on the shared trust service. | integration | small | KIT-A3-F5-P1, KIT-A3-F5-P2, KIT-A3-F16-P1, svc-trust |  |
| SOC-A15-F9-P1 | **No-advert rule**: A co-op rule that bans adverts and data sales, changeable only under the protected-rules process (SOC-A22-F1-P3); an advert is a placement the platform sells, so creators' own labelled sponsorships (SOC-A15-F6) are not adverts. | legal | small | svc-voting, SOC-A22-F1-P3 | [K01](../kit/STATUS.md) |
| SOC-A15-F10-P1 | **App store payment rules**: When Apple and Google require their own in-app purchase, with a 15 to 30 percent cut, for memberships, paid videos, tips and storage fees; which exemptions apply, such as Apple's rule on optional gifts that pass in full to the receiver (guideline 3.2.1, current text to be checked); and where linking out is allowed, such as the US since May 2025. | legal | medium |  | [K04](../kit/STATUS.md) |
| SOC-A15-F10-P2 | **Purchase plan per country**: For each storefront, buy on the web only, link out where allowed, or use reader-app style access, so a store's cut is never hidden in a creator's price. | service | medium | SOC-A15-F10-P1, SOC-A15-F2-P1 |  |
| SOC-A15-F11-P1 | **Tip and membership fraud checks**: Card testing, stolen-card tips and self-tipping caught with the kit's fraud screening and published risk rules (KIT-A3-F13-P1, KIT-A3-F13-P2). | integration | small | KIT-A3-F13-P1, KIT-A3-F13-P2, SOC-A15-F1-P1 |  |
| SOC-A15-F11-P2 | **Payout holds and negative balances**: New creators' first payouts are held for a published time, and negative balances after chargebacks are recovered within the kit's limits (KIT-A3-F16-P2). | integration | small | KIT-A3-F16-P2, SOC-A15-F4-P1 |  |
| SOC-A15-F11-P3 | **Anti-money-laundering and sanctions**: Whether anti-money-laundering rules for online marketplaces cover creator tips and memberships (example: New Zealand's DIA rules; unverified), under the kit's programme (KIT-A4-F5-P1), and sanctions screening of payouts through the provider (KIT-A4-F8-P1). | legal | small | KIT-A4-F5-P1 | [K02](../kit/STATUS.md) |
| SOC-A15-F12-P1 | **Revenue splits**: Agreed shares of tips, memberships and sales for co-authored posts, channel teams and series collaborators, each paid out separately. | service | medium | SOC-A15-F4-P1, SOC-A3-F10-P1 |  |
| SOC-A15-F12-P2 | **Gift memberships**: Buy a membership for someone else, who can accept or decline it. | service | small | SOC-A15-F2-P1 |  |
| SOC-A15-F13-P1 | **Overseas VAT on digital sales**: Works out, collects and returns VAT or GST where EU, UK, Australian and other platform rules make the co-op responsible for electronic services, using the kit's tax rules per country (KIT-A4-F7-P1); needs a tax adviser's view. | service | medium | KIT-A4-F7-P1, SOC-A15-F5-P3 |  |

## SOC-A16 Creator studio and stats

One place for creators to see how their work is doing without tracking anyone, manage everything they have posted, handle comments and messages, work as a team, deal with copyright claims, and publish straight from Spare Studio.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A16-F1** See how your work is doing | See views, watch time, how long people stay, and followers gained, for each post and over time. | core | P1 Private counting; P2 Stats dashboard; P3 Safe audience summaries; P4 Export your stats |
| **SOC-A16-F2** Manage all your content | Search, filter and bulk-edit your posts and videos: visibility, captions, playlists and licence. | next | P1 Content manager |
| **SOC-A16-F3** One inbox for comments and messages | Read and answer comments, mentions and messages from one place, with filters. | next | P1 Creator inbox |
| **SOC-A16-F4** Work as a team | Give editors, managers and moderators access to your channel without sharing your login. | next | P1 Channel team roles |
| **SOC-A16-F5** Handle copyright claims | See claims against your videos, answer them, and track the outcome. | next | P1 Claims centre |
| **SOC-A16-F6** Publish from Spare Studio | Send a finished video or photo from Spare Studio straight to your channel. | later | P1 Publish through the command API; P2 Bulk upload |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A16-F1-P1 | **Private counting**: Views and watch time counted in aggregate on the kit's private usage counts (KIT-A22-F8-P1), with no per-person tracking kept. | integration | medium | KIT-A22-F8-P1, kit-store |  |
| SOC-A16-F1-P2 | **Stats dashboard**: Per-post and per-channel views, watch time, retention curve and followers gained. | interface | medium | SOC-A16-F1-P1 |  |
| SOC-A16-F1-P3 | **Safe audience summaries**: Audience summaries such as country or language only for groups large enough that no one can be singled out. | library | small | SOC-A16-F1-P1 |  |
| SOC-A16-F1-P4 | **Export your stats**: Download stats as CSV or JSON. | service | small | SOC-A16-F1-P1 |  |
| SOC-A16-F2-P1 | **Content manager**: A table of everything you posted with filters and bulk actions, each an operation you can undo, with a link to fix a published video (SOC-A6-F10). | interface | medium | kit-ops, SOC-A3-F1-P1 |  |
| SOC-A16-F3-P1 | **Creator inbox**: Comments, mentions and messages together, with filters for unanswered, members and keywords. | interface | medium | SOC-A3-F4-P2, SOC-A9-F1-P1 |  |
| SOC-A16-F4-P1 | **Channel team roles**: Team roles with a log of who did what. | service | small | SOC-A1-F4-P3 |  |
| SOC-A16-F5-P1 | **Claims centre**: Your copyright notices, counter-notices and appeals in one place. | interface | small | SOC-A13-F4-P1 |  |
| SOC-A16-F6-P1 | **Publish through the command API**: Spare Studio publishes using Spare Social's public command API, not shared code; the post goes to the section its AI-use answer gives (SOC-A11-F3-P2), never to the AI section just because software posted it. | integration | small | SOC-A23-F1-P1, SOC-A11-F3-P2 |  |
| SOC-A16-F6-P2 | **Bulk upload**: Upload many videos at once with details, including each one's AI-use answer, from a spreadsheet. | interface | medium | SOC-A17-F1-P1, SOC-A11-F1-P1 |  |

## SOC-A17 Media pipeline and delivery

The machinery under every photo and video: uploads that survive bad connections, transcoding into sizes for streaming, codec choices that avoid patent costs, image processing, storage and delivery at cost, automatic captions, safe handling of harmful files and links, fair use of storage, and deletion that really deletes.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A17-F1** Upload anything reliably | Upload photos and long videos that pause and resume on a bad connection, with hidden location data removed. | core | P1 Resumable uploads; P2 Upload checks; P3 Strip hidden data; P4 Original kept |
| **SOC-A17-F2** Videos play smoothly anywhere | Watch any video that starts fast and adjusts to your connection, from slow rural mobile to fibre. | core | P1 Transcoding ladder; P2 Codec choice and patents; P3 Streaming packaging; P4 Thumbnails and preview strips; P5 Even loudness; P6 Transcoding job queue |
| **SOC-A17-F3** Photos look right and load fast | See sharp photos in the right colours that load quickly, even on slow connections. | core | P1 Image sizes and formats; P2 Colour and HDR photos; P3 Blur-up placeholders |
| **SOC-A17-F4** Delivered at cost | Get videos and photos from storage and delivery that cost as little as possible, with the costs published. | core | P1 Storage with tiers; P2 Delivery network choice; P3 Peer-assisted playback; P4 Signed media links; P5 Metering for the ledger |
| **SOC-A17-F5** Automatic captions | Get captions made for every video and voice note, then fix them if needed. | core | P1 Speech to text; P2 Caption files; P3 Spoken language detection |
| **SOC-A17-F6** Deleted means deleted | When you delete a photo or video, every copy, size and cached piece on Spare Social's servers is gone within a published time, and other servers are asked to delete theirs. | core | P1 Deletion fan-out; P2 Legal hold; P3 Deletes sent across the network |
| **SOC-A17-F7** Media handled safely | Know a booby-trapped file or link cannot break into Spare Social's servers. | core | P1 Sandboxed media processing; P2 Safe outside fetches |
| **SOC-A17-F8** Fair use of storage | Know storage is kept for sharing, not for free file hosting, pirated films or huge uploads nobody watches. | next | P1 Storage abuse rules |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A17-F1-P1 | **Resumable uploads**: Chunked uploads that resume after a drop, on the kit's one file service (KIT-A15-F7-P1) using the open tus protocol, with limits sized for long videos. | integration | medium | KIT-A15-F7-P1 |  |
| SOC-A17-F1-P2 | **Upload checks**: File type, size and quota checks from the kit's file service (KIT-A15-F7-P1), plus Spare Social's checks on video and audio files inside the sandbox (SOC-A17-F7-P1). | integration | small | KIT-A15-F7-P1, SOC-A17-F7-P1 |  |
| SOC-A17-F1-P3 | **Strip hidden data**: Uses the kit's photo metadata stripping (KIT-A15-F7-P3) and does the same for videos: GPS location and device details removed by default. | library | small | KIT-A15-F7-P3, engine-media |  |
| SOC-A17-F1-P4 | **Original kept**: Keep the original file for export and for transcoding again later. | data | medium | kit-store |  |
| SOC-A17-F2-P1 | **Transcoding ladder**: Turn each upload into several sizes and bitrates for streaming. | service | large | engine-media | [M03](STATUS.md) |
| SOC-A17-F2-P2 | **Codec choice and patents**: Compare AV1, VP9, H.264 and HEVC on quality, device support, cost and patent licensing. | research | medium |  | [M03](STATUS.md) |
| SOC-A17-F2-P3 | **Streaming packaging**: Package videos for adaptive streaming with HLS and DASH. | library | medium | engine-media | [M03](STATUS.md) |
| SOC-A17-F2-P4 | **Thumbnails and preview strips**: Make thumbnails and scrub-bar previews for every video. | library | small | engine-media |  |
| SOC-A17-F2-P5 | **Even loudness**: Level audio so videos play at a steady volume. | library | small | engine-media |  |
| SOC-A17-F2-P6 | **Transcoding job queue**: Queue and workers for transcoding, recording machine time per video for the cost ledger. | service | medium | kit-ops, svc-ledger | [M06](STATUS.md) |
| SOC-A17-F3-P1 | **Image sizes and formats**: Photo sizes from the kit's file service (KIT-A15-F7-P1), served as AVIF and WebP with JPEG as a fallback. | library | medium | KIT-A15-F7-P1, engine-media |  |
| SOC-A17-F3-P2 | **Colour and HDR photos**: Keep colour profiles and HDR photos correct from camera to screen. | library | medium | engine-media | [S03](../spare-studio/STATUS.md) |
| SOC-A17-F3-P3 | **Blur-up placeholders**: Tiny previews that show while the full photo loads. | library | small | SOC-A17-F3-P1 |  |
| SOC-A17-F4-P1 | **Storage with tiers**: Object storage with hot and cold tiers, moving old, rarely watched files to cheaper storage. | service | large | kit-store | [M06](STATUS.md) |
| SOC-A17-F4-P2 | **Delivery network choice**: Compare co-op caches, delivery networks bought at cost and member servers on cost and reach in each country. | research | medium |  | [M06](STATUS.md) |
| SOC-A17-F4-P3 | **Peer-assisted playback**: Viewers share pieces of videos with each other to cut bandwidth, written as our own code. | library | large | SOC-A17-F2-P3 | [M06](STATUS.md) |
| SOC-A17-F4-P4 | **Signed media links**: Time-limited links from the kit's file service and sharing rules (KIT-A15-F7-P1, KIT-A15-F8-P1), so private and members-only media cannot be passed around. | integration | small | KIT-A15-F7-P1, KIT-A15-F8-P1 |  |
| SOC-A17-F4-P5 | **Metering for the ledger**: Measure storage and bandwidth per program, per creator and per member for the open cost ledger. | service | medium | svc-ledger | [K06](../kit/STATUS.md) |
| SOC-A17-F5-P1 | **Speech to text**: Speech recognition through the kit's captions (KIT-A19-F4-P1): an openly licensed model run at cost, in many languages, starting with the example country's (for New Zealand: English and te reo Māori). | integration | medium | KIT-A19-F4-P1 | [M03](STATUS.md) |
| SOC-A17-F5-P2 | **Caption files**: WebVTT and SRT caption files with correct timing, from the kit's captions library (KIT-A19-F4-P1). | integration | small | KIT-A19-F4-P1 |  |
| SOC-A17-F5-P3 | **Spoken language detection**: Detect the language spoken so captions use the right model. | library | small | SOC-A17-F5-P1 |  |
| SOC-A17-F6-P1 | **Deletion fan-out**: Removes every size, copy, delivery cache and peer piece, on top of the kit's deletion (KIT-A18-F2-P1), and confirms when done. | service | medium | KIT-A18-F2-P1, KIT-A15-F7-P1, SOC-A17-F4-P1 |  |
| SOC-A17-F6-P2 | **Legal hold**: Keep evidence only when the law requires it, such as reported child abuse material, sealed and logged. | legal | medium |  | [M02](STATUS.md) |
| SOC-A17-F6-P3 | **Deletes sent across the network**: Sends delete requests to every server a post reached, and says plainly in the privacy statement that servers on the open network may keep copies. | service | small | SOC-A17-F6-P1, SOC-A18-F1-P3, SOC-A18-F1-P4 | [M01](STATUS.md) |
| SOC-A17-F7-P1 | **Sandboxed media processing**: Transcoders and media parsers run in a locked-down sandbox with no network access, so a harmful video file cannot take over a server. | service | medium | SOC-A17-F2-P1 |  |
| SOC-A17-F7-P2 | **Safe outside fetches**: Link previews, the remote content cache and feed imports refuse internal and private addresses, follow few redirects and cap size and time, so no one can use them to reach our own systems (server-side request forgery). | library | small | kit-core |  |
| SOC-A17-F8-P1 | **Storage abuse rules**: Published rules against using video hosting as free file storage, hosting pirated films or making huge uploads nobody watches, with quotas (SOC-A17-F1-P2) and warnings before removal. | legal | small | SOC-A13-F2-P1, SOC-A17-F1-P2 | [M06](STATUS.md) |

## SOC-A18 Open network and moving between networks

Speaking the open social protocols so members can follow and be followed across other open networks, take their followers and posts with them, use their own web address, run their own servers under clear rules, and see outside posts safely.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A18-F1** Follow across open networks | Follow people on other open networks and be followed by them, from day one. | core | P1 Protocol decision; P2 Handles and discovery; P3 ActivityPub server; P4 AT Protocol server; P5 Post type mapping; P6 Promises each protocol cannot carry |
| **SOC-A18-F2** Take your followers with you | Move your account to another server or network and keep your followers. | core | P1 Account move; P2 Member-held keys |
| **SOC-A18-F3** Run your own Spare Social server | Let a member group run its own server that joins the network. | next | P1 Self-host package; P2 Server-to-server moderation; P3 Rules for using the name |
| **SOC-A18-F4** See outside posts safely | View posts and media from other networks with the same filters and protections as our own. | core | P1 Remote content cache; P2 Forward reports |
| **SOC-A18-F5** Learn from existing open networks | Build on the lessons of existing open networks without copying their code. | core | P1 Study open networks; P2 Protocol library licence check |
| **SOC-A18-F6** Take your posts with you | Move your posts, videos and photos to another server or network along with your account. | next | P1 Move posts and media |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A18-F1-P1 | **Protocol decision**: ActivityPub, the AT Protocol, both, or our own, compared on portability, reach, moderation and licences. | research | medium |  | [M01](STATUS.md) |
| SOC-A18-F1-P2 | **Handles and discovery**: Find accounts across networks by handle, using WebFinger and DNS-based handles, built with the kit's open network identity (KIT-A1-F9-P1), which must move to core for this. | service | small | SOC-A18-F1-P1, KIT-A1-F9-P1 | [M01](STATUS.md) |
| SOC-A18-F1-P3 | **ActivityPub server**: Actors, inboxes and outboxes, signed requests and delivery queues for the W3C standard. | service | large | SOC-A18-F1-P1, kit-ops | [M01](STATUS.md) |
| SOC-A18-F1-P4 | **AT Protocol server**: A personal data repository per member, portable identity, and reading the network's relay. | service | large | SOC-A18-F1-P1, kit-sync | [M01](STATUS.md) |
| SOC-A18-F1-P5 | **Post type mapping**: Map short videos, long videos, photos, stories, events and groups onto each protocol's objects. | library | large | SOC-A3-F1-P1 | [M08](STATUS.md) |
| SOC-A18-F1-P6 | **Promises each protocol cannot carry**: Lists the Spare Social promises each protocol cannot carry, such as AT Protocol repositories making follows, likes and blocks public and lacking private accounts, which clash with private accounts, hidden blocks, hidden counts and teen contact limits. | research | small | SOC-A18-F1-P1 | [M01](STATUS.md) |
| SOC-A18-F2-P1 | **Account move**: Moving accounts and followers with each protocol's own move method. | service | medium | SOC-A18-F1-P3, SOC-A18-F1-P4 | [M01](STATUS.md) |
| SOC-A18-F2-P2 | **Member-held keys**: Signing keys and identity the member can export and take with them, on the kit's open network identity (KIT-A1-F9-P1), which must move to core for this. | integration | medium | KIT-A1-F9-P1 | [M01](STATUS.md) |
| SOC-A18-F3-P1 | **Self-host package**: Container images, set-up guide and updates for running a server. | service | medium | SOC-A22-F5-P1 |  |
| SOC-A18-F3-P2 | **Server-to-server moderation**: Server block lists, shared labels and published reasons for cutting off a server. | service | medium | SOC-A13-F5-P1, SOC-A12-F4-P1 | [M04](STATUS.md) |
| SOC-A18-F3-P3 | **Rules for using the name**: What a member-run server must meet to carry the Spare Social name (legal duties, known-content checks, response times) and what happens when it does not, as part of the steward's name licence (KIT-A14-F12-P1) and the kit's self-host agreement duties (KIT-A22-F7-P4). | legal | small | SOC-A13-F2-P1, KIT-A22-F7-P4 | [K01](../kit/STATUS.md) |
| SOC-A18-F4-P1 | **Remote content cache**: Fetch, cache and proxy outside posts and media with size limits and checks, through the safe fetcher (SOC-A17-F7-P2). | service | medium | SOC-A17-F4-P1, SOC-A13-F3-P1, SOC-A17-F7-P2 |  |
| SOC-A18-F4-P2 | **Forward reports**: Send reports about outside posts to the server they came from. | service | small | SOC-A13-F1-P2 |  |
| SOC-A18-F5-P1 | **Study open networks**: How PeerTube, Pixelfed, Mastodon and Bluesky are built and where they struggle; AGPL code is read only, never copied. | research | large |  | [M08](STATUS.md) |
| SOC-A18-F5-P2 | **Protocol library licence check**: Check the licences of AT Protocol and ActivityPub libraries before any use. | legal | small |  | [M08](STATUS.md) |
| SOC-A18-F6-P1 | **Move posts and media**: Move posts, videos and photos with an account, following the draft LOLA portability spec, since ActivityPub's move keeps followers but not content. | service | medium | SOC-A18-F2-P1, SOC-A19-F1-P1 | [M01](STATUS.md) |

## SOC-A19 Your data and privacy

Downloading everything in open formats, bringing your history and lists from other networks, privacy settings and a check-up, your legal rights to see and correct your data, no tracking or adverts, keeping your posts out of AI training, and keeping data in the right country.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A19-F1** Download all your data | Download every post, original photo and video, comment, follow and setting in open formats you can read. | core | P1 Export builder; P2 Export screens; P3 Export chats from your device |
| **SOC-A19-F2** Bring your history from other networks | Import your old posts, photos and videos from the download files other networks give you. | next | P1 Archive importers; P2 Import legal check; P3 Lists in common formats |
| **SOC-A19-F3** Privacy settings | Choose who can find you, see your follows, tag you, see when you are online, and whether search engines list you. | core | P1 Privacy settings page; P2 Discoverability rules; P3 See your profile as others do; P4 Privacy check-up |
| **SOC-A19-F4** Your rights under the law | Ask to see, correct or delete what Spare Social holds about you and get an answer within the legal time. | core | P1 Privacy request desk; P2 Breach plan; P3 Privacy statement; P4 Notices for data from others |
| **SOC-A19-F5** No tracking and no adverts | Use Spare Social with no third-party trackers, no advertising profile and no data sold. | core | P1 Tracker-free checks; P2 Data register |
| **SOC-A19-F6** Data kept in the right place | Know where your data is stored: in the regions the co-op runs, starting with New Zealand, or in your own region where your law asks. | core | P1 Data residency research |
| **SOC-A19-F7** See and clear your activity | See your likes, comments, searches and watch history in one place and delete any of it. | core | P1 Activity log |
| **SOC-A19-F8** Keep your posts out of AI training | Know your posts are never used to train AI unless you say yes, and have that choice published in a form other sites' machines read. | next | P1 AI training opt-out; P2 Bulk scraping limits |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A19-F1-P1 | **Export builder**: Spare Social's posts, original media, comments, follows and settings plug into the kit's data export (KIT-A18-F1-P1), in JSON (ActivityStreams where it fits) with a simple offline viewer. | integration | medium | KIT-A18-F1-P1, SOC-A17-F1-P4 |  |
| SOC-A19-F1-P2 | **Export screens**: Ask for an export, choose what to include, and get notified when it is ready. | interface | small | SOC-A19-F1-P1, svc-messaging |  |
| SOC-A19-F1-P3 | **Export chats from your device**: Encrypted chats are exported from the member's own device, since the server cannot read them. | library | medium | SOC-A9-F8-P1 |  |
| SOC-A19-F2-P1 | **Archive importers**: Read the export files other networks give their own users, keeping original dates. | library | large | SOC-A3-F1-P1 |  |
| SOC-A19-F2-P2 | **Import legal check**: Check that reading each network's export files fits our legal rules before building an importer. | legal | small |  | [M02](STATUS.md) |
| SOC-A19-F2-P3 | **Lists in common formats**: Import and export follows, blocks, mutes, lists and bookmarks in common open formats, such as Mastodon's CSV files and AT Protocol repositories. | library | medium | SOC-A2-F1-P1, SOC-A18-F1-P1 | [M01](STATUS.md) |
| SOC-A19-F3-P1 | **Privacy settings page**: One clear page for every privacy choice, with private-by-default options. | interface | small | SOC-A12-F5-P1 |  |
| SOC-A19-F3-P2 | **Discoverability rules**: Search engine listing, contact matching and suggestions (SOC-A2-F4) all follow your choices. | service | small | SOC-A1-F5-P2 |  |
| SOC-A19-F3-P3 | **See your profile as others do**: View your profile as the public, a follower or a friend sees it. | interface | small | SOC-A1-F2-P2, SOC-A3-F2-P1 |  |
| SOC-A19-F3-P4 | **Privacy check-up**: A guided walk through your audience and privacy choices, with plain advice on each. | interface | small | SOC-A19-F3-P1 |  |
| SOC-A19-F4-P1 | **Privacy request desk**: Access, correction and deletion requests through the kit's privacy page and request clock (KIT-A18-F3-P1, KIT-A18-F3-P2), tracked against each country's deadlines (example: New Zealand's Privacy Act 2020). | integration | small | KIT-A18-F3-P1, KIT-A18-F3-P2, SOC-A19-F1-P1 | [M02](STATUS.md) |
| SOC-A19-F4-P2 | **Breach plan**: Uses the kit's breach response (KIT-A18-F7-P1), tested against Spare Social's own data. | integration | small | KIT-A18-F7-P1 |  |
| SOC-A19-F4-P3 | **Privacy statement**: Spare Social's privacy statement on the kit's terms and privacy policy (KIT-A18-F4-P2): what is collected, why and for how long, in plain words, including that servers on the open network may keep copies. | legal | small | KIT-A18-F4-P2, SOC-A19-F5-P2 | [M02](STATUS.md) |
| SOC-A19-F4-P4 | **Notices for data from others**: Notices under each country's privacy law (example: New Zealand's principle 3A, in force since 1 May 2026), for data about people collected from others, such as uploaded contacts, tags and mentions of non-members, using the kit's collection notices (KIT-A18-F5-P1). | legal | small | KIT-A18-F5-P1 | [M02](STATUS.md) |
| SOC-A19-F5-P1 | **Tracker-free checks**: Build checks that block third-party scripts and trackers from every page and app. | service | small |  |  |
| SOC-A19-F5-P2 | **Data register**: Spare Social's entries in the kit's retention schedules and processor list (KIT-A18-F6-P1, KIT-A18-F15-P2), published as one list of every piece of personal data, why it is kept and when it is deleted. | integration | small | KIT-A18-F6-P1, KIT-A18-F15-P2 |  |
| SOC-A19-F6-P1 | **Data residency research**: Where members' data must or should live, feeding the kit's data location service (KIT-A18-F9-P1). | research | medium |  | [M02](STATUS.md) |
| SOC-A19-F7-P1 | **Activity log**: Your own actions listed by type and date, searchable, with bulk delete. | interface | small | kit-ops |  |
| SOC-A19-F8-P1 | **AI training opt-out**: Uses the kit's AI training choice (KIT-A17-F11-P1), off until the member says yes, and publishes machine-readable reservations for public posts, such as robots rules and a text-and-data-mining reservation under the EU copyright directive (article 4), sent with posts over the open network where a protocol allows. | service | small | KIT-A17-F11-P1, SOC-A19-F3-P1, SOC-A18-F1-P5 |  |
| SOC-A19-F8-P2 | **Bulk scraping limits**: Uses the kit's bulk copy limits (KIT-A16-F1-P4) to stop the API and public pages being harvested. | integration | small | KIT-A16-F1-P4 |  |

## SOC-A20 Apps, devices, offline and notifications

Using Spare Social in a browser, on phones, on computers and on the TV; links that open the right post; a first-run guide; notifications you choose; working offline and on slow connections; and settings that follow you.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A20-F1** Use it in a browser | Use everything from a web browser and install it like an app. | core | P1 Web app shell; P2 Installable web app |
| **SOC-A20-F2** Phone apps | Use Spare Social on Android and iPhone with the camera, background uploads and the share sheet. | core | P1 Phone app approach; P2 App store rules; P3 Phone integrations; P4 Links that open the right post |
| **SOC-A20-F3** Notifications you choose | Choose which notifications reach you, how (push, email, digest) and when. | core | P1 Notification types and settings; P2 Notifications inbox; P3 Digests |
| **SOC-A20-F4** Offline and slow connections | Write posts offline that send later, download videos to watch offline, and save data on slow mobile. | core | P1 Offline drafts and queue; P2 Offline video downloads; P3 Data saver |
| **SOC-A20-F5** Settings that follow you | Your settings, theme and text size are the same on every device. | core | P1 Settings sync; P2 Light and dark themes |
| **SOC-A20-F6** Watch on the big screen | Watch on a TV, and send a video from your phone to the TV. | later | P1 TV app; P2 Casting research; P3 Sign in on a TV with a code |
| **SOC-A20-F7** Computer app | Use a desktop app for big uploads, long editing sessions and live streaming. | later | P1 Desktop shell; P2 Linux desktop app |
| **SOC-A20-F8** Learn your choices on day one | Get a short first-run guide to feeds, the Human and AI sections and filters, so you know what you can choose. | next | P1 First-run guide |
| **SOC-A20-F9** Use a phone without Google services | Get notifications on a phone that runs without Google's services. | later | P1 Push without Google |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A20-F1-P1 | **Web app shell**: Spare Social's navigation and routes on the kit's shared components and app frame (KIT-A20-F1-P1, KIT-A20-F1-P2). | integration | medium | KIT-A20-F1-P1, KIT-A20-F1-P2 |  |
| SOC-A20-F1-P2 | **Installable web app**: Install from the browser through the kit's installable apps (KIT-A20-F2-P2), with offline caching and push where the browser allows. | integration | small | KIT-A20-F2-P2, SOC-A20-F1-P1 | [K04](../kit/STATUS.md) |
| SOC-A20-F2-P1 | **Phone app approach**: Web app, Tauri mobile or native, decided with live camera, upload and push needs in mind. | research | medium |  | [K04](../kit/STATUS.md) |
| SOC-A20-F2-P2 | **App store rules**: What each app store requires of social apps with user posts, such as reporting, blocking and age ratings, on top of the kit's app store rules (KIT-A20-F8-P1), plus the payment rules for paid content (SOC-A15-F10-P1). | legal | small | KIT-A20-F8-P1 | [K04](../kit/STATUS.md) |
| SOC-A20-F2-P3 | **Phone integrations**: Camera, background upload, share to and from other apps, and photo library access. | interface | medium | SOC-A20-F2-P1 | [K04](../kit/STATUS.md) |
| SOC-A20-F2-P4 | **Links that open the right post**: Links to a post or profile open it in the app, or on the web without the app, using the kit's deep links (KIT-A20-F4-P1). | integration | small | KIT-A20-F4-P1 |  |
| SOC-A20-F3-P1 | **Notification types and settings**: Every Spare Social notification type with its own switch for push, email and digest, on the kit's notification settings (KIT-A8-F5-P1). | integration | medium | KIT-A8-F5-P1, svc-messaging |  |
| SOC-A20-F3-P2 | **Notifications inbox**: One list of notifications grouped by post and type. | interface | small | SOC-A20-F3-P1 |  |
| SOC-A20-F3-P3 | **Digests**: Daily or weekly email summaries instead of instant alerts. | service | small | svc-messaging, SOC-A20-F3-P1 |  |
| SOC-A20-F4-P1 | **Offline drafts and queue**: Posts, comments and messages queue offline and send when back online. | library | medium | kit-sync |  |
| SOC-A20-F4-P2 | **Offline video downloads**: Save videos to watch offline where the creator allows it. In the phone and computer apps, saved copies are encrypted and need a licence check to play, so they stop when access ends; a plain browser cannot enforce that, so members-only videos are not saved there. | library | medium | SOC-A17-F4-P4 |  |
| SOC-A20-F4-P3 | **Data saver**: Smaller sizes and no preloading when data saver is on; it turns autoplay off through the one autoplay setting (SOC-A21-F4-P1). | library | small | SOC-A17-F2-P3, SOC-A21-F4-P1 |  |
| SOC-A20-F5-P1 | **Settings sync**: Spare Social's settings on the kit's settings framework (KIT-A20-F5-P1), stored as operations and synced to every device. | integration | small | KIT-A20-F5-P1, kit-sync |  |
| SOC-A20-F5-P2 | **Light and dark themes**: Light and dark themes from the kit's shared components (KIT-A20-F1-P1), following the device or your choice. | interface | small | KIT-A20-F1-P1 |  |
| SOC-A20-F6-P1 | **TV app**: A TV layout that works with a remote. | interface | large | SOC-A6-F2-P1 |  |
| SOC-A20-F6-P2 | **Casting research**: Open casting standards and the terms of each platform's own casting kit. | research | medium |  |  |
| SOC-A20-F6-P3 | **Sign in on a TV with a code**: Sign in on a TV by entering a short code on your phone, using the kit's device sign-in (KIT-A1-F12-P1). | integration | small | SOC-A20-F6-P1, KIT-A1-F12-P1 |  |
| SOC-A20-F7-P1 | **Desktop shell**: The kit's desktop wrapper (KIT-A20-F2-P2) with direct file access for big uploads. | integration | small | KIT-A20-F2-P2 |  |
| SOC-A20-F7-P2 | **Linux desktop app**: A Linux build of the desktop app, ready for Spare OS. | interface | small | SOC-A20-F7-P1 |  |
| SOC-A20-F8-P1 | **First-run guide**: Spare Social's first-run guide on the kit's guide screens (KIT-A20-F6-P1): pick feeds, sections and filters, explained for people used to a For You feed. | integration | small | KIT-A20-F6-P1, SOC-A10-F2-P3 |  |
| SOC-A20-F9-P1 | **Push without Google**: Push through UnifiedPush for phones without Google's services, alongside Apple's and Google's push. | integration | small | SOC-A20-F3-P1 |  |

## SOC-A21 Accessibility and languages

Usable by everyone: screen readers and keyboards, alt text and descriptions, captions styled your way, comfort settings, flashing warnings, the interface in many languages, starting with the example country's (for New Zealand: English and te reo Māori), sign language such as New Zealand Sign Language, translation of posts, and local formats.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A21-F1** Works with screen readers and keyboards | Use every screen, including players and feeds, with a screen reader, a keyboard or switch control. | core | P1 Accessibility standard and checks; P2 Accessible players and feeds |
| **SOC-A21-F2** Alt text and descriptions | Add alt text to every image, with an optional suggestion made on the device, and audio descriptions for videos. | core | P1 Alt text field and reminder; P2 Image description suggestions; P3 Audio description tracks |
| **SOC-A21-F3** Captions your way | Turn captions on everywhere and set their size, colour and background. | core | P1 Caption style settings |
| **SOC-A21-F4** Comfortable to use | Reduce motion, stop autoplay, raise contrast and enlarge text. | core | P1 Comfort settings |
| **SOC-A21-F5** Your language | Use the interface in many languages, starting with the example country's (for New Zealand: English and te reo Māori), including right-to-left scripts. | core | P1 Interface translation system; P2 Fluent speaker translation; P3 Right-to-left and complex scripts; P4 Community translation |
| **SOC-A21-F6** Read posts in your language | Translate posts, comments and captions into your language with one tap. | next | P1 Open machine translation |
| **SOC-A21-F7** Local formats | See dates, times, numbers and money the way your country writes them, in your time zone. | core | P1 Locale formatting |
| **SOC-A21-F8** Warnings for flashing video | Get a warning before a video with flashing that could trigger a seizure, and turn on a filter that softens it. | next | P1 Flash detection and filter |
| **SOC-A21-F9** Sign language | Watch videos and lives with a sign language interpreter in a small window, and mark posts as sign language, such as New Zealand Sign Language. | later | P1 Interpreter window; P2 Sign language as a post language |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A21-F1-P1 | **Accessibility standard and checks**: Meet WCAG 2.2 AA with the kit's accessible components and testing (KIT-A19-F2-P1, KIT-A19-F2-P2), adding checks for the players, swipe feed and stories. | integration | small | KIT-A19-F2-P1, KIT-A19-F2-P2 |  |
| SOC-A21-F1-P2 | **Accessible players and feeds**: Labelled controls, focus order and keyboard shortcuts in the video player, swipe feed and stories. | interface | medium | SOC-A5-F1-P1, SOC-A6-F2-P1 |  |
| SOC-A21-F2-P1 | **Alt text field and reminder**: An alt text box on every image from the kit's image descriptions (KIT-A19-F2-P3), and a gentle reminder before posting without it. | interface | small | SOC-A3-F1-P2, KIT-A19-F2-P3 |  |
| SOC-A21-F2-P2 | **Image description suggestions**: An openly licensed model on the device suggests alt text the poster can edit; a suggestion is marked as AI-made until edited (SOC-A11-F1-P3). | library | medium | SOC-A11-F1-P3 |  |
| SOC-A21-F2-P3 | **Audio description tracks**: An extra audio track that describes what is on screen. | library | medium | engine-media, SOC-A6-F5-P3 |  |
| SOC-A21-F3-P1 | **Caption style settings**: Size, colour, background and position for captions. | interface | small | SOC-A17-F5-P2 |  |
| SOC-A21-F4-P1 | **Comfort settings**: Reduced motion, high contrast and text size from the kit's display settings (KIT-A19-F3-P1), plus the one autoplay setting that every player and feed obeys. | interface | small | KIT-A19-F3-P1, SOC-A20-F5-P1 |  |
| SOC-A21-F5-P1 | **Interface translation system**: Every interface string in the kit's translation framework (KIT-A19-F1-P1), an open format such as Fluent. | integration | small | KIT-A19-F1-P1 |  |
| SOC-A21-F5-P2 | **Fluent speaker translation**: Translation of the example country's languages by fluent speakers, paid fairly, with a glossary agreed with them, starting with te reo Māori for New Zealand, through the kit's member translation (KIT-A19-F1-P3). | data | medium | KIT-A19-F1-P3, KIT-A18-F16-P1, SOC-A21-F5-P1 |  |
| SOC-A21-F5-P3 | **Right-to-left and complex scripts**: Mirrored layouts and correct shaping for every script, from the kit's right-to-left layout (KIT-A19-F7-P1). | integration | medium | KIT-A19-F7-P1, engine-text | [S05](../spare-studio/STATUS.md) |
| SOC-A21-F5-P4 | **Community translation**: Members suggest and review interface translations through the kit's member translation (KIT-A19-F1-P3). | integration | small | KIT-A19-F1-P3, SOC-A21-F5-P1 |  |
| SOC-A21-F6-P1 | **Open machine translation**: Uses the kit's machine translation at cost (KIT-A19-F10-P2), labelled, with the original one tap away, run on the device where a model fits. | integration | medium | KIT-A19-F10-P2 |  |
| SOC-A21-F7-P1 | **Locale formatting**: Dates, times, numbers, currency and time zones per locale from the kit's local formats (KIT-A19-F1-P2). | integration | small | KIT-A19-F1-P2 |  |
| SOC-A21-F8-P1 | **Flash detection and filter**: Finds flashing that breaks WCAG 2.3.1 when a video is processed, adds a warning, and offers a filter that softens it. | library | medium | engine-media, SOC-A17-F2-P1 |  |
| SOC-A21-F9-P1 | **Interpreter window**: A second video track for a sign language interpreter, such as for New Zealand Sign Language, shown picture-in-picture in videos and live streams. | library | medium | SOC-A6-F2-P1, SOC-A7-F1-P1 |  |
| SOC-A21-F9-P2 | **Sign language as a post language**: Each sign language, such as New Zealand Sign Language, as a language posts can carry, for filters and search, alongside the kit's sign language versions of key screens (KIT-A19-F8-P1). | data | small | SOC-A3-F1-P7, KIT-A19-F8-P1 |  |

## SOC-A22 Running the co-op: rules, costs, admin and support

How members run Spare Social: who belongs and votes, protected core promises, the open cost ledger for storage and bandwidth, covering video costs fairly, help and support, running the servers safely, the legal set-up, insurance and duties in other countries, and measuring performance.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A22-F1** Members set the rules | Propose and vote on Spare Social's rules, moderator elections and any fees. | core | P1 Proposals and votes; P2 Who belongs and who votes; P3 Protected core promises |
| **SOC-A22-F2** See what it costs | See each month what storage, bandwidth, transcoding and moderation cost, and what that means per member. | core | P1 Social cost lines on the ledger; P2 Your share shown |
| **SOC-A22-F3** Cover video costs fairly | Use Spare Social free, with heavy uploaders paying their share of storage at cost if members vote for that. | next | P1 Cost-sharing decision; P2 Allowances and billing |
| **SOC-A22-F4** Get help | Find answers in a help centre, ask for support and see if something is down. | core | P1 Help centre; P2 Support requests; P3 Status page; P4 Urgent help and other ways to reach us |
| **SOC-A22-F5** Run the servers safely | Admins run, watch, back up and secure the servers, with every admin action logged. | core | P1 Admin console; P2 Monitoring and alerts; P3 Backups and tested restores; P4 Security reviews and reporting |
| **SOC-A22-F6** Legal set-up | Read plain terms of use and know which legal body runs the service. | core | P1 Terms of use; P2 Licence register |
| **SOC-A22-F7** Measured performance | Know how fast feeds load and videos start, measured and published. | core | P1 Performance budgets |
| **SOC-A22-F8** Costs and duties of a public platform | See the co-op insured against claims over what members post, its duties in other countries costed, researchers given fair access, public posts archived by national libraries, and complaints about the co-op itself heard. | next | P1 Media liability insurance; P2 Overseas duties on the ledger; P3 Access for researchers; P4 Archiving by the National Library; P5 Complaints about the co-op |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A22-F1-P1 | **Proposals and votes**: Spare Social proposals, quorum and results through the kit's ballots (KIT-A12-F3-P1), with one vote per person on the member register (KIT-A14-F2-P1), never per profile, pseudonymous account or agent, applied as operations. | integration | small | svc-voting, kit-ops, KIT-A12-F3-P1, KIT-A14-F2-P1 | [K01](../kit/STATUS.md) |
| SOC-A22-F1-P2 | **Who belongs and who votes**: Defines who is a member of Spare Social, who counts as its workers (creators, moderators, translators), what viewers vote on, and how members of local co-ops in other countries and users on other servers take part, using the kit's electorates (KIT-A12-F5-P2). | legal | medium | KIT-A12-F5-P2 | [K01](../kit/STATUS.md) |
| SOC-A22-F1-P3 | **Protected core promises**: The promises to remove only what the law requires and to carry no adverts can change only by a supermajority or a change to the constitution, so a simple majority cannot vote away lawful minority speech. | legal | small | KIT-A12-F5-P1 | [K01](../kit/STATUS.md) |
| SOC-A22-F2-P1 | **Social cost lines on the ledger**: Monthly cost lines for gigabytes stored and served, hours transcoded and moderation hours. | data | medium | svc-ledger, SOC-A17-F4-P5 | [K06](../kit/STATUS.md) |
| SOC-A22-F2-P2 | **Your share shown**: Each member sees what their own use cost the co-op this month. | interface | small | SOC-A22-F2-P1 | [K06](../kit/STATUS.md) |
| SOC-A22-F3-P1 | **Cost-sharing decision**: Compare a free allowance, an at-cost storage fee and voluntary contributions for covering video costs. | research | medium |  | [M06](STATUS.md) |
| SOC-A22-F3-P2 | **Allowances and billing**: Storage allowances and at-cost billing through the shared payments service, if members choose it. | integration | medium | svc-payments, SOC-A22-F3-P1 | [K02](../kit/STATUS.md) |
| SOC-A22-F4-P1 | **Help centre**: Spare Social's plain-language help pages in the kit's help centre (KIT-A21-F1-P1), in every interface language. | data | medium | KIT-A21-F1-P1 |  |
| SOC-A22-F4-P2 | **Support requests**: Ask for help and get tracked replies from people, on the kit's support cases (KIT-A21-F2-P1). | integration | small | KIT-A21-F2-P1 |  |
| SOC-A22-F4-P3 | **Status page**: Spare Social's uptime, incidents and fixes on the kit's status page (KIT-A21-F5-P1). | integration | small | KIT-A21-F5-P1 |  |
| SOC-A22-F4-P4 | **Urgent help and other ways to reach us**: Urgent cases routed to a person on call (KIT-A21-F3-P1), and email, phone and relay services (example: New Zealand Relay) for members who find apps hard to use (KIT-A21-F4-P1). | integration | small | KIT-A21-F3-P1, KIT-A21-F4-P1 |  |
| SOC-A22-F5-P1 | **Admin console**: Spare Social's admin actions on the kit's admin console (KIT-A22-F6-P1), every action an operation with a stated reason. | integration | medium | KIT-A22-F6-P1 |  |
| SOC-A22-F5-P2 | **Monitoring and alerts**: Spare Social's services on the kit's monitoring and alerts (KIT-A22-F1-P1), with uptime published. | integration | small | KIT-A22-F1-P1 |  |
| SOC-A22-F5-P3 | **Backups and tested restores**: The kit's backups and restore drills (KIT-A22-F3-P1) extended to media storage, with restores tested on a schedule. | integration | medium | KIT-A22-F3-P1, SOC-A17-F4-P1 |  |
| SOC-A22-F5-P4 | **Security reviews and reporting**: Outside security reviews of Spare Social before launch, through the kit's security reviews and reporting page (KIT-A22-F5-P3). | integration | small | KIT-A22-F5-P3 |  |
| SOC-A22-F6-P1 | **Terms of use**: Plain-language terms that match the published rules and the co-op's legal entity. | legal | medium | SOC-A13-F2-P1 | [K01](../kit/STATUS.md) |
| SOC-A22-F6-P2 | **Licence register**: Spare Social's entries in the repository's one licence register (piece A14): every open-source dependency, its licence and where it came from. | legal | small |  | [A14](../spare-construct/STATUS.md) |
| SOC-A22-F7-P1 | **Performance budgets**: Targets for feed load time, video start time and upload speed, measured on real connections, starting with New Zealand's. | research | small |  |  |
| SOC-A22-F8-P1 | **Media liability insurance**: Insurance for defamation and copyright claims over what members post, compared on cost and cover, with the premium on the cost ledger (KIT-A14-F17-P1). | research | small | KIT-A14-F17-P1 |  |
| SOC-A22-F8-P2 | **Overseas duties on the ledger**: Costs of overseas legal representatives, regulator fees and compliance work shown as their own lines on the cost ledger. | data | small | SOC-A22-F2-P1, SOC-A13-F3-P11 | [K06](../kit/STATUS.md) |
| SOC-A22-F8-P3 | **Access for researchers**: Vetted researchers can study public data under published terms that protect members' privacy. | service | small | SOC-A13-F5-P1 |  |
| SOC-A22-F8-P4 | **Archiving by the National Library**: Public posts can be archived by national libraries that collect web content, such as the National Library of New Zealand, with private and deleted posts never included. | integration | small | SOC-A3-F2-P1 |  |
| SOC-A22-F8-P5 | **Complaints about the co-op**: Complaints about Spare Social or the co-op itself, not about a post, through the kit's complaints process (KIT-A21-F9-P2), with the outside route that applies. | integration | small | KIT-A21-F9-P2 |  |

## SOC-A23 Headless API, plugins and AI agents

Everything a person can do, a script or AI agent can do through the same commands: the command and query API, AI access through MCP with labels, guards and approvals, actions only people may take, plugins for feeds, labels and effects, a developer API, embeds, webhooks, and a register of where Spare Social uses AI.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **SOC-A23-F1** Everything by command | Do anything in Spare Social through the same commands the app uses, from a script or another program. | core | P1 Spare Social command set; P2 Query API |
| **SOC-A23-F2** Use your own AI | Let your own AI assistant read your feeds and post or manage for you, within permissions you set, with its posts marked as AI. | core | P1 MCP server; P2 Agent actions labelled; P3 Guards against hidden instructions; P4 Approval and undo; P5 People-only actions |
| **SOC-A23-F3** Plugins | Add plugins for custom feeds, labels, filters, editor effects and new post types, each showing what it can access. | next | P1 Plugin points; P2 Plugin directory |
| **SOC-A23-F4** Developer API | Build apps on Spare Social with sign-in through Spareware, scoped permissions, rate limits and good docs. | next | P1 Public API with scopes; P2 API docs and examples |
| **SOC-A23-F5** Show your posts elsewhere | Put your videos and posts on your own website with an embed or a feed. | next | P1 Embed player and widgets |
| **SOC-A23-F6** Webhooks | Get a call to your own server when you post, get a comment or gain a member. | later | P1 Webhooks |
| **SOC-A23-F7** Small games and apps | Play small games and use mini apps with friends inside Spare Social, each sandboxed with its permissions shown. | later | P1 Games and mini apps |
| **SOC-A23-F8** See where Spare Social uses AI | See every place Spare Social uses AI in decisions about you, and ask for a person to review one. | next | P1 AI use register |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| SOC-A23-F1-P1 | **Spare Social command set**: Every action (post, follow, react, report, upload, moderate) as a command on the kit's operation layer; the route a post takes never changes its section (SOC-A11-F3-P2). | library | large | kit-ops, SOC-A3-F1-P1 |  |
| SOC-A23-F1-P2 | **Query API**: Read feeds, profiles, posts and search through one query API that respects every audience. | library | large | SOC-A3-F2-P1, SOC-A10-F4-P2 |  |
| SOC-A23-F2-P1 | **MCP server**: Spare Social's commands on the kit's MCP server (KIT-A17-F1-P1), meeting MCP authorisation (KIT-A17-F1-P3), with permissions the member grants and can take back; encrypted chats are reached only through the member's own device (SOC-A9-F11-P1). | integration | medium | kit-mcp, KIT-A17-F1-P1, KIT-A17-F1-P3, SOC-A23-F1-P1 |  |
| SOC-A23-F2-P2 | **Agent actions labelled**: Anything an agent does through MCP carries an agent label naming the person responsible (KIT-A17-F9-P1); a post's section still follows its AI-use answer (SOC-A11-F3-P2). | service | small | SOC-A11-F3-P2, KIT-A17-F9-P1 | [M09](STATUS.md) |
| SOC-A23-F2-P3 | **Guards against hidden instructions**: Feeds, comments and messages reach an agent marked as data, never as instructions, using the kit's guards (KIT-A17-F5-P1), with each agent's own rate, scope and spending limits (KIT-A17-F4-P1). | integration | small | KIT-A17-F5-P1, KIT-A17-F4-P1, SOC-A23-F2-P1 |  |
| SOC-A23-F2-P4 | **Approval and undo**: A person approves before an agent deletes an account or posts, tips, pays or messages a stranger, and sees every agent action in a log with undo, using the kit's approvals and agent activity (KIT-A17-F2-P1, KIT-A17-F3-P1). | integration | small | KIT-A17-F2-P1, KIT-A17-F3-P1, SOC-A23-F2-P1 |  |
| SOC-A23-F2-P5 | **People-only actions**: Context-note ratings, poll votes, moderator elections, appeals and bulk reports can be made only by a person, never an agent, on the kit's people-only actions (KIT-A17-F9-P2). | integration | small | KIT-A17-F9-P2 |  |
| SOC-A23-F3-P1 | **Plugin points**: Spare Social's plugin hooks for feeds, labellers, filters, effects and post types, on the kit's plugin host (KIT-A16-F5-P2). | library | large | kit-plugin, KIT-A16-F5-P2 |  |
| SOC-A23-F3-P2 | **Plugin directory**: Spare Social's plugins in the kit's one plugin directory (KIT-A16-F6-P1), with permissions shown before install. | integration | small | KIT-A16-F6-P1, SOC-A23-F3-P1 |  |
| SOC-A23-F4-P1 | **Public API with scopes**: Sign-in through the kit's provider (KIT-A1-F6-P1), scoped tokens and published rate limits (KIT-A16-F1-P2, KIT-A16-F1-P3), and versions with notice before anything is removed (KIT-A16-F9-P1). | integration | medium | KIT-A1-F6-P1, KIT-A16-F1-P2, KIT-A16-F1-P3, KIT-A16-F9-P1, SOC-A23-F1-P1 |  |
| SOC-A23-F4-P2 | **API docs and examples**: Reference docs and working examples, generated with the kit's developer docs (KIT-A16-F3-P2). | data | medium | KIT-A16-F3-P2, SOC-A23-F4-P1 |  |
| SOC-A23-F5-P1 | **Embed player and widgets**: A small embeddable player and post widgets with no tracking, offered through the oEmbed standard most sites use to embed posts. | interface | small | SOC-A3-F1-P1, SOC-A6-F2-P1 |  |
| SOC-A23-F6-P1 | **Webhooks**: Spare Social events, such as a new post, comment or member, on the kit's signed webhooks (KIT-A16-F2-P1). | integration | small | KIT-A16-F2-P1 |  |
| SOC-A23-F7-P1 | **Games and mini apps**: Games and mini apps as sandboxed plugins that can read only what the member allows. | library | large | SOC-A23-F3-P1 |  |
| SOC-A23-F8-P1 | **AI use register**: Spare Social's uses of AI (spam signals, image blur, detection hints, captions, translation and alt text) in the kit's public register (KIT-A17-F10-P1), with review by a person on request (KIT-A17-F10-P2). | integration | small | KIT-A17-F10-P1, KIT-A17-F10-P2 |  |

## Questions for assessing this map

- Would the core features alone make a credible switch for someone leaving TikTok, YouTube, Instagram or Facebook, and which one should the first release aim at?
- Is a time-order feed plus a discovery feed with published rules enough to keep people who are used to TikTok's For You feed?
- Can the co-op afford video storage and bandwidth at cost (piece M06), and would members accept a free allowance with at-cost storage for heavy uploaders?
- Which open protocol comes first (piece M01), which Spare Social promises can it not carry (SOC-A18-F1-P6), and is moving your followers really part of the first release?
- Is the line between personal filters and legal removal clear, and does 'remove only what the law requires' hold up against each country's duties, starting with New Zealand's, and other governments' demands (pieces M02, SOC-A13-F3-P7)?
- Can elected volunteer moderators meet 24 and 48-hour legal deadlines around the clock, or does the co-op need paid, vetted staff on call (SOC-A13-F3-P8, SOC-A13-F7)?
- Should lawful adult sexual content be allowed with full age and consent checks, kept out of the app-store apps, or ruled out (SOC-A13-F2-P4)?
- How does Spare Social meet minimum-age laws, such as Australia's under-16 law and New Zealand's August 2026 bill, without keeping identity documents?
- Is short video workable without commercial music, and what would licensing music cost at cost in each country, starting with New Zealand?
- Is end-to-end encrypted messaging right for the first release, and is reporting from the reporter's own device enough for safety?
- Who belongs to Spare Social and who votes on its rules: creators, moderators, viewers, members in other countries, users on other servers (SOC-A22-F1-P2)?
- Can creators really keep everything except payment fees on iPhone and Android, given the app stores' payment rules (SOC-A15-F10)?
- Can the Human and AI sections be enforced fairly, and should a bot account that uses no AI, such as a quake alert account, still post only in the AI section as ARCHITECTURE.md says (piece M09)?
- Which candidate kit parts should move down next: live chat, rich text, private contact discovery, hidden words, labelling services and the one end-to-end encryption library shared with Spare Office?
- Which parts carry legal risk under 'Built legally, or not at all': archive importers, codecs, casting kits, GIF providers, open model licences, note-ranking code, video hash libraries?
- Is the commitment to many languages, starting with the example country's (for New Zealand: English and te reo Māori), funded, with fluent speakers paid to translate?
- Is this map the right size for contributors with spare tokens, and what is the smallest slice worth building and demonstrating first?
- What numbers would prove the demo works, such as video start time on rural mobile networks, such as in rural New Zealand, feed load time and cost per member per month?
