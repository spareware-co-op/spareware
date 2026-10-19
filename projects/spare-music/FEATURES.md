# Spare Music: everything to build

*Stage: Mapping. Generated from [features.json](features.json); edit that file, not this one. The architecture is in [ARCHITECTURE.md](ARCHITECTURE.md) and the work board in [STATUS.md](STATUS.md).*

Spare Music replaces Spotify, Apple Music, SoundCloud and Bandcamp with one place to listen to music and to buy it straight from the artists. Artists choose how their music is sold and keep everything except the running costs, shown on the open cost ledger. Listeners choose the Human section, the AI section or both, songwriters are paid through their collecting societies country by country, uploads are fingerprinted so nobody can sell someone else's music, and only what the law requires is removed. This map lists everything a full replacement needs, top down: 22 areas, 175 features a person can use, and 320 buildable parts. 116 features are core, meaning a first usable release needs them; the rest are next (soon after) or later. Each part names what it depends on: shared kit crates (kit-), kit services (svc-), shared engines (engine-), parts of the shared kit by their KIT- ID (see the [kit map](../kit/FEATURES.md)) or other Spare Music parts by ID. Where a part waits on a research or decision piece, the piece links to its work board.

## Areas at a glance

| Area | Features | Core | Parts |
| --- | --- | --- | --- |
| [MUS-A1 Joining, accounts and artist pages](#mus-a1-joining-accounts-and-artist-pages) | 8 | 8 | 21 |
| [MUS-A2 Listening and the player](#mus-a2-listening-and-the-player) | 10 | 6 | 21 |
| [MUS-A3 Your library and playlists](#mus-a3-your-library-and-playlists) | 8 | 4 | 13 |
| [MUS-A4 Finding music](#mus-a4-finding-music) | 9 | 5 | 16 |
| [MUS-A5 Human and AI sections](#mus-a5-human-and-ai-sections) | 8 | 5 | 17 |
| [MUS-A6 Uploading and releasing music](#mus-a6-uploading-and-releasing-music) | 10 | 7 | 20 |
| [MUS-A7 Rights and splits](#mus-a7-rights-and-splits) | 10 | 7 | 20 |
| [MUS-A8 Collecting societies and licences per country](#mus-a8-collecting-societies-and-licences-per-country) | 7 | 4 | 13 |
| [MUS-A9 Paying for music](#mus-a9-paying-for-music) | 14 | 8 | 27 |
| [MUS-A10 Earnings, payouts and tax](#mus-a10-earnings-payouts-and-tax) | 7 | 5 | 14 |
| [MUS-A11 Stopping copies and stolen uploads](#mus-a11-stopping-copies-and-stolen-uploads) | 8 | 5 | 20 |
| [MUS-A12 Artists and fans together](#mus-a12-artists-and-fans-together) | 8 | 4 | 11 |
| [MUS-A13 Artist studio and stats](#mus-a13-artist-studio-and-stats) | 8 | 4 | 10 |
| [MUS-A14 Moderation, legal duties and transparency](#mus-a14-moderation-legal-duties-and-transparency) | 10 | 9 | 22 |
| [MUS-A15 Personal controls, young people and wellbeing](#mus-a15-personal-controls-young-people-and-wellbeing) | 6 | 4 | 9 |
| [MUS-A16 Audio pipeline, storage and delivery](#mus-a16-audio-pipeline-storage-and-delivery) | 7 | 6 | 16 |
| [MUS-A17 Open network and moving](#mus-a17-open-network-and-moving) | 5 | 2 | 8 |
| [MUS-A18 Your data and privacy](#mus-a18-your-data-and-privacy) | 5 | 5 | 7 |
| [MUS-A19 Apps, devices and offline](#mus-a19-apps-devices-and-offline) | 7 | 5 | 8 |
| [MUS-A20 Accessibility and languages](#mus-a20-accessibility-and-languages) | 7 | 5 | 7 |
| [MUS-A21 Running it: fees, votes, costs and support](#mus-a21-running-it-fees-votes-costs-and-support) | 7 | 6 | 11 |
| [MUS-A22 Headless API, plugins and AI agents](#mus-a22-headless-api-plugins-and-ai-agents) | 6 | 2 | 9 |

Kinds of part: **library** (code compiled into the app), **service** (runs on servers), **interface** (screens), **data** (records and formats), **integration** (joins a kit service or outside provider), **research** and **legal** (notes and checks before building). Sizes follow the work board: small, medium, large.

## MUS-A1 Joining, accounts and artist pages

Joining with one Spareware login, listening before signing up, a listener profile, artist pages for solo artists, bands and labels, proving an artist is who they say, running a label or a team, getting back in, and leaving or handing on a catalogue.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A1-F1** Join with one Spareware login | Join in under a minute with a passkey and a name, using the same login as every other Spareware app. | core | P1 Spareware login hook-up; P2 Sign-up flow; P3 Sign-up spam guard |
| **MUS-A1-F2** Listen before signing up | Play free music and previews with no account, and keep what you saved when you join. | core | P1 Listening without an account; P2 Previews for everyone |
| **MUS-A1-F3** Make a listener profile | Choose a name, a photo and what others can see, such as your public playlists and the artists you support. | core | P1 Listener profile record; P2 Listener page |
| **MUS-A1-F4** Make an artist page | Set up a page for yourself, your band or your label, with a photo, a bio, links, members and your releases. | core | P1 Artist record; P2 Artist page; P3 Band members and roles; P4 Artist name rules |
| **MUS-A1-F5** Prove you are the artist | Get a verified badge that shows the page is run by the artist or their team, so nobody can pose as them. | core | P1 Artist verification; P2 Verified artist badge; P3 Claim an existing page |
| **MUS-A1-F6** Run a label or a team | Run several artists from one label account, give staff roles, and see every action in a log. | core | P1 Label accounts; P2 Team action log |
| **MUS-A1-F7** Get back into your account | Recover a lost login and sign out of devices you do not recognise. | core | P1 Recovery; P2 Sessions and payout guard |
| **MUS-A1-F8** Leave or hand on your music | Delete a listener account, take your music down, or name who looks after an artist's catalogue and earnings after death. | core | P1 Delete and take down; P2 Estates and heirs; P3 Settle up before closing |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A1-F1-P1 | **Spareware login hook-up**: Use the kit's account service and passkey sign-in (KIT-A1-F2-P1), with passwords as a fallback. | integration | small | KIT-A1-F1-P1, KIT-A1-F2-P1 |  |
| MUS-A1-F1-P2 | **Sign-up flow**: Screens for a name, the Human and AI choice (MUS-A5-F2-P2) and agreeing to the published rules, with the kit's minimum age check (KIT-A1-F1-P5). | interface | small | MUS-A1-F1-P1, KIT-A1-F1-P5, MUS-A14-F2-P1, MUS-A5-F2-P2 |  |
| MUS-A1-F1-P3 | **Sign-up spam guard**: Uses the kit's sign-up abuse protection (KIT-A1-F1-P4), so fake accounts cannot be made in bulk to farm plays. | integration | small | KIT-A1-F1-P4 |  |
| MUS-A1-F2-P1 | **Listening without an account**: Uses the kit's local profile (KIT-A1-F15-P1) to keep likes, the queue and playlists on the device, moved into the account on sign-up. | integration | small | KIT-A1-F15-P1 |  |
| MUS-A1-F2-P2 | **Previews for everyone**: Each release sets which tracks play in full and how long the preview is for the rest, before anyone signs in. | service | small | MUS-A9-F1-P1, MUS-A16-F3-P2 |  |
| MUS-A1-F3-P1 | **Listener profile record**: Name, photo, a short bio and what is shown (playlists, follows, artists supported), stored as operations on the kit's profile model (KIT-A1-F5-P1). | data | small | KIT-A1-F5-P1, kit-ops |  |
| MUS-A1-F3-P2 | **Listener page**: A page with public playlists and the artists the listener chose to show. Listening history is never shown unless the listener turns it on. | interface | small | MUS-A1-F3-P1 |  |
| MUS-A1-F4-P1 | **Artist record**: An artist is a solo act, a band, a duo, an orchestra or a label, with its own name, bio, links, members, genres and an optional home town, separate from any login. | data | medium | KIT-A1-F5-P1, kit-ops |  |
| MUS-A1-F4-P2 | **Artist page**: One page with releases, popular tracks, shows, a support button and the artist's posts, laid out the same way for every artist, with no paid placement. | interface | medium | MUS-A1-F4-P1 |  |
| MUS-A1-F4-P3 | **Band members and roles**: Several logins run one artist page as owner, manager or member, on the kit's roles and team management (KIT-A1-F5-P2, KIT-A1-F7-P2). | integration | small | MUS-A1-F4-P1, KIT-A1-F5-P2, KIT-A1-F7-P2 |  |
| MUS-A1-F4-P4 | **Artist name rules**: Two artists may share a name, as in real life, so pages are told apart by town, genre and photo; look-alikes of verified artists are checked for impersonation (MUS-A11-F4-P1). | service | small | MUS-A1-F4-P1, MUS-A11-F4-P1 |  |
| MUS-A1-F5-P1 | **Artist verification**: Checks that the person running a page is the artist or acts for them: the kit's identity check (KIT-A2-F1-P2), links from the artist's own website and other pages, and for labels the business check (KIT-A2-F7-P1). | integration | medium | KIT-A2-F1-P2, KIT-A2-F7-P1, MUS-A1-F4-P1 |  |
| MUS-A1-F5-P2 | **Verified artist badge**: Shown on the page and every release, using the kit's verified badge (KIT-A2-F5-P1), with published rules for how it is earned and lost. | interface | small | MUS-A1-F5-P1, KIT-A2-F5-P1 |  |
| MUS-A1-F5-P3 | **Claim an existing page**: An artist claims a page that a past upload created, and the old uploads are reviewed under the rights rules. | service | small | MUS-A1-F5-P1, MUS-A7-F1-P1 |  |
| MUS-A1-F6-P1 | **Label accounts**: A label is a kit organisation (KIT-A1-F7-P1) that runs several artist pages and releases, with its own payout details. | integration | small | KIT-A1-F7-P1, MUS-A1-F4-P1 |  |
| MUS-A1-F6-P2 | **Team action log**: Every upload, price change, split change and payout change by anyone on the team is logged, using the kit's organisation audit log (KIT-A1-F7-P7). | integration | small | KIT-A1-F7-P7 |  |
| MUS-A1-F7-P1 | **Recovery**: Recovery codes, a backup passkey and assisted recovery from the kit (KIT-A1-F4-P1, KIT-A1-F4-P2). | integration | small | KIT-A1-F4-P1, KIT-A1-F4-P2 |  |
| MUS-A1-F7-P2 | **Sessions and payout guard**: See every signed-in device, and sign in again before payout details or splits change (KIT-A1-F3-P4, KIT-A3-F3-P4). | integration | small | KIT-A1-F3-P2, KIT-A1-F3-P4, KIT-A3-F3-P4 |  |
| MUS-A1-F8-P1 | **Delete and take down**: Delete a listener account with a full purge after a short grace period; artists can take releases down, while buyers keep the downloads they paid for. | service | medium | KIT-A18-F2-P1, MUS-A16-F6-P1 |  |
| MUS-A1-F8-P2 | **Estates and heirs**: When an artist dies their releases stay up, and earnings go to the estate once it proves its right, using the kit's after-death handling (KIT-A18-F11-P1). | integration | medium | KIT-A18-F11-P1, MUS-A10-F2-P1 |  |
| MUS-A1-F8-P3 | **Settle up before closing**: An artist closing an account is paid what is owed first, through the kit's settle-up step (KIT-A18-F2-P2). | integration | small | KIT-A18-F2-P2 |  |

## MUS-A2 Listening and the player

Playing music well on every device: the player, the queue, albums without gaps and at an even volume, sound quality you choose, lyrics and credits while you listen, offline listening, speakers and cars, and listening together.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A2-F1** Play music | Play a track, an album or a playlist, pause, skip and seek, and pick up where you left off on any device. | core | P1 Player; P2 Pick up where you left off; P3 Media keys and lock screen |
| **MUS-A2-F2** Queue and shuffle | See what plays next, reorder it, add to it, shuffle and repeat. | core | P1 Queue; P2 Queue screen |
| **MUS-A2-F3** Albums played the way the artist meant | Hear albums with no gaps between tracks that run together, at an even volume, in the artist's order. | core | P1 Gapless playback; P2 Even loudness; P3 The artist's order kept |
| **MUS-A2-F4** Choose sound quality | Pick normal, high or lossless quality, separately for mobile data, wi-fi and downloads. | core | P1 Quality settings; P2 Lossless streaming |
| **MUS-A2-F5** Lyrics and credits while you listen | Read the words as the song plays, and see who wrote, played and produced each track. | core | P1 Lyrics; P2 Credits panel; P3 Lyrics rights |
| **MUS-A2-F6** Listen offline | Download albums and playlists to play with no signal, kept up to date. | core | P1 Offline downloads; P2 Offline rule |
| **MUS-A2-F7** Play on speakers, TVs and in the car | Send music to speakers and TVs at home, and play it in the car. | next | P1 Casting to speakers; P2 Car play; P3 Sign in on a TV or speaker |
| **MUS-A2-F8** Sleep timer and listening extras | Stop after a set time, change the speed for talks and audiobooks, and crossfade between songs in playlists. | next | P1 Player extras |
| **MUS-A2-F9** Listen together | Listen in time with friends in a shared room, with one queue anyone can add to. | later | P1 Shared listening rooms |
| **MUS-A2-F10** Music videos and visualisers | Watch an artist's music video or a visualiser alongside the track. | later | P1 Music videos |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A2-F1-P1 | **Player**: Play, pause, skip and seek, a waveform you can scrub, and the now-playing screen, on the kit's audio engine (KIT-A27-F8-P4). | interface | medium | KIT-A27-F8-P4, MUS-A16-F3-P2, MUS-A16-F2-P3 |  |
| MUS-A2-F1-P2 | **Pick up where you left off**: The queue and the place in the track follow the listener across devices as operations, so a phone carries on what the laptop was playing. | service | small | kit-sync, MUS-A2-F2-P1 |  |
| MUS-A2-F1-P3 | **Media keys and lock screen**: Play from headphone buttons, keyboard media keys, the lock screen and notifications, with artwork and track names. | integration | small | MUS-A2-F1-P1, KIT-A20-F2-P2 |  |
| MUS-A2-F2-P1 | **Queue**: An editable queue with play next, add to queue, shuffle that plays everything once before repeating, and repeat one or all. | library | small | MUS-A16-F1-P1 |  |
| MUS-A2-F2-P2 | **Queue screen**: The queue as a list you can drag to reorder, with what is playing now and where the queue came from. | interface | small | MUS-A2-F2-P1 |  |
| MUS-A2-F3-P1 | **Gapless playback**: No silence between tracks that run together, using the encoder delay and padding stored with each file. | library | medium | KIT-A27-F8-P4, MUS-A16-F3-P1 |  |
| MUS-A2-F3-P2 | **Even loudness**: Volume kept even from the loudness measured on upload (MUS-A16-F2-P2), by track for playlists and by album for albums, so quiet songs stay quiet; listeners can turn it off. | library | small | MUS-A16-F2-P2, MUS-A2-F1-P1 |  |
| MUS-A2-F3-P3 | **The artist's order kept**: An artist can mark an album as one piece, such as a live set or a classical work, so it plays in order even when shuffle is on. | data | small | MUS-A6-F2-P1 |  |
| MUS-A2-F4-P1 | **Quality settings**: Quality per connection and for downloads, with the data used per hour shown for each. | interface | small | MUS-A16-F3-P2 |  |
| MUS-A2-F4-P2 | **Lossless streaming**: Lossless FLAC at the quality the artist uploaded, where the device and connection can take it. | service | medium | MUS-A16-F3-P2, MUS-A16-F3-P1 |  |
| MUS-A2-F5-P1 | **Lyrics**: Lyrics the songwriter supplies, timed line by line in the open LRC format where they choose, shown in step with the music. | data | small | MUS-A6-F2-P1 |  |
| MUS-A2-F5-P2 | **Credits panel**: Every credit on the track: writers, performers, producers, engineers and any samples, each linked to its page. | interface | small | MUS-A6-F4-P1 |  |
| MUS-A2-F5-P3 | **Lyrics rights**: Lyrics are words with their own copyright, so only the songwriter or their publisher may supply them; the rights declaration covers them. | legal | small | MUS-A7-F1-P1 |  |
| MUS-A2-F6-P1 | **Offline downloads**: Saves music to the device at the chosen quality, kept in step with the library, with the space used shown and one place to clear it (KIT-A15-F4-P1). | service | medium | MUS-A16-F3-P2, KIT-A15-F4-P1 |  |
| MUS-A2-F6-P2 | **Offline rule**: How long offline copies of streamed music last without going online, which follows the copy-protection decision in ARCHITECTURE.md; bought music is the buyer's to keep with no limit. | research | small |  |  |
| MUS-A2-F7-P1 | **Casting to speakers**: Plays on speakers and TVs through open protocols such as UPnP and DLNA, and through makers' casting kits only where their terms allow. | integration | medium | MUS-A2-F1-P1 |  |
| MUS-A2-F7-P2 | **Car play**: Plays through the phone systems built into cars, meeting each one's rules for screens used while driving (KIT-A20-F11-P1). | integration | medium | MUS-A2-F1-P1, KIT-A20-F11-P1 |  |
| MUS-A2-F7-P3 | **Sign in on a TV or speaker**: Sign in on a device with no keyboard using the kit's device sign-in (KIT-A1-F12-P1). | integration | small | KIT-A1-F12-P1 |  |
| MUS-A2-F8-P1 | **Player extras**: Sleep timer, crossfade for playlists (never inside an album that runs together) and speed control for spoken word. | library | small | MUS-A2-F1-P1, MUS-A2-F3-P1 |  |
| MUS-A2-F9-P1 | **Shared listening rooms**: A room where everyone hears the same moment, with a shared queue and chat, through the kit's live updates and chat (KIT-A15-F5-P1, KIT-A8-F1-P2). | service | medium | MUS-A2-F1-P1, KIT-A15-F5-P1, KIT-A8-F1-P2 |  |
| MUS-A2-F10-P1 | **Music videos**: A video the artist uploads with a track, played with the kit's video decoding (KIT-A27-F8-P2), and easy to switch off to save data. | integration | medium | MUS-A6-F2-P1, KIT-A27-F8-P2 |  |

## MUS-A3 Your library and playlists

Saving music, one library of everything you bought or saved, playlists you make, share or make with friends, listening history you control, playing your own files, and bringing your library from another service.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A3-F1** Save music to your library | Save tracks, albums and artists, and see everything you bought, saved or downloaded in one place. | core | P1 Library; P2 Library screens |
| **MUS-A3-F2** Make playlists | Make a playlist, add a note to any track, reorder it, and share it or keep it private. | core | P1 Playlist record; P2 Playlist editor; P3 Who made it |
| **MUS-A3-F3** Share a playlist | Share a link anyone can open, and follow other people's playlists. | core | P1 Playlist links; P2 Follow playlists |
| **MUS-A3-F4** Make playlists together | Let friends add to a playlist and see who added what. | next | P1 Shared playlists |
| **MUS-A3-F5** See and clear your history | See what you played, pause your history, and delete any of it. | core | P1 Listening history |
| **MUS-A3-F6** Play your own files | Add music files you already own and play them beside Spare Music. | next | P1 Your own files |
| **MUS-A3-F7** Bring your library from another service | Move your playlists and saved music from another service, matched to what is on Spare Music. | next | P1 Playlist import; P2 Other services' exports |
| **MUS-A3-F8** Smart playlists and folders | Sort playlists into folders, and make playlists that fill themselves by rules you set. | later | P1 Smart playlists |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A3-F1-P1 | **Library**: Saved tracks, albums, artists and playlists, with bought music marked, sorted the way you choose. | data | small | MUS-A16-F1-P1, kit-ops |  |
| MUS-A3-F1-P2 | **Library screens**: Lists and a grid with filters for bought, saved, downloaded, and Human or AI. | interface | small | MUS-A3-F1-P1, MUS-A5-F2-P1 |  |
| MUS-A3-F2-P1 | **Playlist record**: Title, description, picture, privacy, and tracks with optional notes, as operations so every change has history and undo (KIT-A15-F2-P1). | data | small | kit-ops, KIT-A15-F2-P1, MUS-A16-F1-P1 |  |
| MUS-A3-F2-P2 | **Playlist editor**: Add, remove and drag to reorder, with search inside the playlist. | interface | small | MUS-A3-F2-P1 |  |
| MUS-A3-F2-P3 | **Who made it**: Every playlist shows who made it: a member, an artist or an agent; agent-made playlists carry the kit's agent label (KIT-A17-F9-P1). | data | small | MUS-A3-F2-P1, KIT-A17-F9-P1 |  |
| MUS-A3-F3-P1 | **Playlist links**: Short links and QR codes from the kit (KIT-A20-F9-P1) that open the playlist in the app or the browser, with a preview card (KIT-A28-F2-P3). | integration | small | KIT-A20-F9-P1, KIT-A28-F2-P3, MUS-A3-F2-P1 |  |
| MUS-A3-F3-P2 | **Follow playlists**: Follow a playlist and see when it changes. | service | small | MUS-A3-F2-P1 |  |
| MUS-A3-F4-P1 | **Shared playlists**: Invite people to add and reorder, each addition showing who added it, on the kit's sharing (KIT-A15-F8-P1). | service | small | MUS-A3-F2-P1, KIT-A15-F8-P1 |  |
| MUS-A3-F5-P1 | **Listening history**: Private by default and kept only as long as payouts and fake-play checks need (MUS-A16-F5-P1), then reduced to counts; pause and delete at any time (KIT-A18-F6-P1). | service | small | MUS-A16-F5-P1, KIT-A18-F6-P1 |  |
| MUS-A3-F6-P1 | **Your own files**: Plays music files on the device in the same library, never uploaded unless you choose, read with the kit's openly licensed decoders (KIT-A27-F8-P2). | library | medium | KIT-A27-F8-P2, MUS-A3-F1-P1 |  |
| MUS-A3-F7-P1 | **Playlist import**: Reads exported playlist files (CSV, M3U and XSPF) and matches tracks by ISRC and title, never by scraping another company's service; tracks not on Spare Music are listed so you can tell the artist. | library | medium | MUS-A16-F1-P1, MUS-A6-F2-P2 |  |
| MUS-A3-F7-P2 | **Other services' exports**: Which services let people export their own playlists and library, and in what files, read from the files the listener downloads under each service's terms. | research | small |  |  |
| MUS-A3-F8-P1 | **Smart playlists**: Playlists built from rules such as genre, year, Human only and not played lately, with the rule shown. | library | small | MUS-A3-F2-P1, MUS-A4-F1-P1 |  |

## MUS-A4 Finding music

Search, browsing by genre, mood and place, every new release from artists you follow, discovery with published rules, radio from a song, charts counted in the open, playlists by people, and recognising a song playing nearby. No paid placement anywhere.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A4-F1** Search everything | Search artists, releases, tracks, playlists, labels and lyrics, spelled any way, with or without accents and macrons. | core | P1 Search service; P2 Search screen |
| **MUS-A4-F2** Browse by genre, mood and place | Browse the genres and moods artists chose, and find music from your town or anywhere. | core | P1 Genres and moods; P2 Local scenes |
| **MUS-A4-F3** New from artists you follow | See every new release from artists you follow, newest first, with nothing left out. | core | P1 Release feed; P2 New releases screen |
| **MUS-A4-F4** Discovery with published rules | Get suggestions from rules you can read, choose or turn off, with no paid placement. | core | P1 Discovery rules; P2 Why this was suggested; P3 No paid placement |
| **MUS-A4-F5** Charts counted in the open | See what is most played and most bought, by genre and country, counted by a published method that leaves out fake plays. | core | P1 Charts; P2 How charts are counted; P3 Chart screens |
| **MUS-A4-F6** Radio from a song | Start an endless mix from a song, an artist or a genre, built by a published rule. | next | P1 Radio |
| **MUS-A4-F7** Playlists by people | Browse playlists made by members, artists and music writers, each signed by its maker. | next | P1 People's playlists to browse |
| **MUS-A4-F8** Reviews and music writing | Read reviews and interviews by members and music writers, with the music playing alongside. | later | P1 Reviews and articles |
| **MUS-A4-F9** Recognise a song playing nearby | Hold up your phone to find a song playing in a cafe or on the radio. | later | P1 Song recognition |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A4-F1-P1 | **Search service**: Music search on the kit's search service (KIT-A6-F1-P2) with accent-aware matching (KIT-A19-F1-P4) and the listener's section filter. | integration | medium | KIT-A6-F1-P2, KIT-A19-F1-P4, MUS-A16-F1-P1, MUS-A5-F2-P1 |  |
| MUS-A4-F1-P2 | **Search screen**: One box with results grouped by artists, releases, tracks, playlists and lyrics, and recent searches you can clear. | interface | small | MUS-A4-F1-P1 |  |
| MUS-A4-F2-P1 | **Genres and moods**: Genres and moods from a published list the artist picks from, plus free tags, with no hidden scoring. | data | small | MUS-A6-F2-P1 |  |
| MUS-A4-F2-P2 | **Local scenes**: Music by place, from the home town an artist chose to show, never from tracking where listeners are. | service | small | MUS-A1-F4-P1, MUS-A4-F1-P1 |  |
| MUS-A4-F3-P1 | **Release feed**: Every release and post from followed artists in time order; nothing is hidden or ranked. | service | small | MUS-A12-F1-P1, MUS-A6-F2-P1 |  |
| MUS-A4-F3-P2 | **New releases screen**: This week's releases from followed artists at the top of home, with a mark on anything new since you last looked. | interface | small | MUS-A4-F3-P1 |  |
| MUS-A4-F4-P1 | **Discovery rules**: Suggestion lists built from published rules, such as listeners of this also played, new in your genres and new from your area, with the rule shown on each list (KIT-A6-F1-P3). | library | medium | KIT-A6-F1-P3, MUS-A16-F5-P1 |  |
| MUS-A4-F4-P2 | **Why this was suggested**: Every suggestion has a Why button naming the rule that picked it. | interface | small | MUS-A4-F4-P1 |  |
| MUS-A4-F4-P3 | **No paid placement**: A protected rule (MUS-A21-F1-P3) that nobody can pay to be suggested, charted or placed higher. | legal | small | MUS-A21-F1-P3 |  |
| MUS-A4-F5-P1 | **Charts**: Weekly charts by plays, by purchases and by number of listeners, per genre and country, from plays that passed the fake-play checks. | service | small | MUS-A16-F5-P1, MUS-A11-F5-P1 |  |
| MUS-A4-F5-P2 | **How charts are counted**: The counting rule, and how many plays were left out as fake, published every week. | data | small | MUS-A4-F5-P1 |  |
| MUS-A4-F5-P3 | **Chart screens**: Charts by genre and country, with the week's movers and the counting rule one tap away. | interface | small | MUS-A4-F5-P1 |  |
| MUS-A4-F6-P1 | **Radio**: A queue that keeps filling by a published rule from similar listeners' plays and shared genres, respecting Human, AI or both. | library | medium | MUS-A4-F4-P1, MUS-A2-F2-P1 |  |
| MUS-A4-F7-P1 | **People's playlists to browse**: Public playlists by genre and mood, made by people, never paid for, each showing who made it. | service | small | MUS-A3-F2-P1, MUS-A3-F2-P3 |  |
| MUS-A4-F8-P1 | **Reviews and articles**: Long posts with players inside, written by members and labelled when paid for. | service | medium | MUS-A16-F1-P1 |  |
| MUS-A4-F9-P1 | **Song recognition**: Matches a few seconds recorded on the phone against the catalogue's fingerprints (MUS-A11-F1-P2), using only a method the patent check cleared. | library | large | MUS-A11-F1-P2, MUS-A11-F1-P1 |  |

## MUS-A5 Human and AI sections

The catalogue has a Human section and an AI section. Artists declare AI use on every track, AI agent accounts are labelled and publish only in the AI section, uploads are checked for content credentials and AI-detection signals as hints only, an AI copy of a real singer's voice needs that singer's consent, listeners choose Human, AI or both, and wrong placements are fixed in the open.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A5-F1** Declare AI use on every track | Say for every track whether AI was used and how: none, AI-assisted or AI-made, with which parts and which tools. | core | P1 AI-use field; P2 Declaration step; P3 Where the line falls |
| **MUS-A5-F2** Listen to Human, AI or both | Choose the Human section, the AI section or both, everywhere you browse, and change it any time. | core | P1 Section filter; P2 Section choice; P3 Mixed playlists |
| **MUS-A5-F3** Labelled AI agents | Know when an account is an AI agent, and who is responsible for it. | core | P1 Agent accounts; P2 Section follows the answer, not the route |
| **MUS-A5-F4** Fix wrong sections in the open | Flag a track in the wrong section; a moderator moves it with the reason in the public log, and the artist can appeal. | core | P1 Wrong-section report; P2 Section move and appeal |
| **MUS-A5-F5** Real voices only with consent | Know an AI copy of a real singer's voice appears only with that singer's consent. | core | P1 Voice consent; P2 Voice and likeness law |
| **MUS-A5-F6** Checked, not assumed | See content credentials when a track carries them, while moderators get detection signals as hints, never verdicts. | next | P1 Content credentials reader; P2 Detection signals research; P3 Signals as hints |
| **MUS-A5-F7** How the AI was trained | See whether the maker of an AI track says the tools were trained with permission. | next | P1 Training declaration |
| **MUS-A5-F8** Mark your music as human-made | Attach signed content credentials to music recorded and mixed with Spareware tools. | later | P1 Sign your own recordings |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A5-F1-P1 | **AI-use field**: None, AI-assisted or AI-made for each track, with the parts that used AI (music, vocals, lyrics, artwork, mixing or mastering) and an optional note on tools. | data | small | MUS-A6-F2-P1 |  |
| MUS-A5-F1-P2 | **Declaration step**: The upload asks every time, with the artist's usual answers filled in, and is never skipped. | interface | small | MUS-A5-F1-P1, MUS-A6-F1-P2 |  |
| MUS-A5-F1-P3 | **Where the line falls**: Published examples of AI-assisted and AI-made, such as an AI mastering tool or a generated drum pattern against a fully generated song, voted on by artists. | legal | small | MUS-A14-F2-P1 |  |
| MUS-A5-F2-P1 | **Section filter**: Every list, search, chart, radio and suggestion filters by section on the server. | service | small | MUS-A5-F1-P1 |  |
| MUS-A5-F2-P2 | **Section choice**: A clear Human, AI or Both switch, chosen at sign-up and changeable any time, the same on every device (KIT-A20-F5-P2). | interface | small | MUS-A5-F2-P1, KIT-A20-F5-P2 |  |
| MUS-A5-F2-P3 | **Mixed playlists**: A playlist with tracks from both sections plays only the section the listener chose, and says how many tracks it skipped. | service | small | MUS-A5-F2-P1, MUS-A3-F2-P1 |  |
| MUS-A5-F3-P1 | **Agent accounts**: Accounts run by AI agents carry a label and the person or organisation responsible, and publish only in the AI section, using the kit's agent labels (KIT-A17-F9-P1). | data | small | KIT-A17-F9-P1, MUS-A1-F4-P1 |  |
| MUS-A5-F3-P2 | **Section follows the answer, not the route**: A track goes to the section its AI-use answer gives, whether it came through the app, the command API, MCP or a distributor feed; an agent's own uploads always go to AI. | service | small | MUS-A5-F1-P1, MUS-A5-F3-P1, MUS-A22-F1-P1 |  |
| MUS-A5-F4-P1 | **Wrong-section report**: A report type for tracks in the wrong section. | data | small | MUS-A14-F1-P1 |  |
| MUS-A5-F4-P2 | **Section move and appeal**: Moderators move a track with a logged reason; the artist can appeal, and repeated false answers follow the published ladder. | service | small | MUS-A14-F4-P1, MUS-A14-F5-P1, MUS-A14-F6-P1 |  |
| MUS-A5-F5-P1 | **Voice consent**: An AI track that imitates a real person's voice needs that person's recorded consent; without it, the track comes down when that person or their estate asks, logged in public. | legal | medium | MUS-A5-F1-P1, MUS-A14-F1-P1 |  |
| MUS-A5-F5-P2 | **Voice and likeness law**: Research on rights in a person's voice and likeness in each country, such as passing off, privacy, personality rights and new laws on digital replicas. | research | medium |  |  |
| MUS-A5-F6-P1 | **Content credentials reader**: Reads C2PA content credentials in audio files (the c2pa-rs library lists MP3, WAV, FLAC and M4A) and in artwork, with the kit's content credentials (KIT-A27-F8-P6), and shows them on the track. | integration | medium | KIT-A27-F8-P6, MUS-A16-F2-P1 |  |
| MUS-A5-F6-P2 | **Detection signals research**: Compares open methods for spotting AI-generated audio on accuracy, bias across genres and languages, and licence, for moderator hints only. | research | medium |  |  |
| MUS-A5-F6-P3 | **Signals as hints**: Detection scores reach moderators beside the artist's answer, never move a track by themselves, and are listed in the AI use register (KIT-A17-F10-P1). | service | small | MUS-A5-F6-P2, KIT-A17-F10-P1, MUS-A14-F7-P2 |  |
| MUS-A5-F7-P1 | **Training declaration**: AI tracks say whether the tools were trained with the rights holders' permission, as the artist declares it; not known is an allowed answer. | data | small | MUS-A5-F1-P1 |  |
| MUS-A5-F8-P1 | **Sign your own recordings**: Adds C2PA content credentials when a track is exported from Spareware tools, signed with the co-op's certificate (KIT-A27-F8-P7). | integration | medium | KIT-A27-F8-P6, KIT-A27-F8-P7, MUS-A5-F6-P1 |  |

## MUS-A6 Uploading and releasing music

Artists upload lossless masters, add the details a release needs, set artwork, credits and lyrics, schedule a release with pre-orders, share private previews, fix mistakes after release and take releases down, from the app, a script or a distributor's feed.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A6-F1** Upload a release | Upload a single, EP or album in lossless quality, with progress that survives a dropped connection. | core | P1 Resumable uploads; P2 Upload screens; P3 Accepted formats |
| **MUS-A6-F2** Release details | Add titles, artists, release date, genre, language, an explicit label and the codes the music trade uses. | core | P1 Release and track records; P2 Industry codes; P3 Explicit and content labels |
| **MUS-A6-F3** Artwork | Add cover art and see how it looks small and large. | core | P1 Cover art; P2 Artwork description |
| **MUS-A6-F4** Credits | Credit everyone who wrote, played, sang, produced, engineered or mixed each track. | core | P1 Credits; P2 Invite collaborators |
| **MUS-A6-F5** Schedule a release | Pick a release day and time, take pre-orders, and choose when previews go live. | core | P1 Release scheduler; P2 Pre-orders |
| **MUS-A6-F6** Share before release | Send a private link to press, radio and friends before release. | core | P1 Private preview links |
| **MUS-A6-F7** Fix or take down a release | Correct a typo, replace a file or take a release down, with buyers told what changed. | core | P1 Release versions; P2 Take down your own music |
| **MUS-A6-F8** Deliver from a distributor or a script | Deliver releases from a distributor in the music trade's standard format, or by script. | next | P1 Distributor feed; P2 Bulk upload by command |
| **MUS-A6-F9** Liner notes and extras | Add liner notes, photos, a booklet and the story behind each song. | next | P1 Liner notes and booklets |
| **MUS-A6-F10** Classical, spoken word and podcasts | Release classical works, audiobooks, talks and podcasts with the details each needs. | later | P1 Classical details; P2 Spoken word and podcasts |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A6-F1-P1 | **Resumable uploads**: Large uploads that pause and resume, on the kit's file uploads (KIT-A15-F7-P1) with an open resumable upload protocol. | integration | medium | KIT-A15-F7-P1 |  |
| MUS-A6-F1-P2 | **Upload screens**: Drag in files, see each one checked, and fill in the release in one guided flow saved as a draft. | interface | medium | MUS-A6-F1-P1, MUS-A6-F2-P1 |  |
| MUS-A6-F1-P3 | **Accepted formats**: Accepts WAV, AIFF and FLAC at CD quality or better, checks for clipping and long silence, and explains any problem in plain words. | library | small | MUS-A16-F2-P1 |  |
| MUS-A6-F2-P1 | **Release and track records**: Release, track and version records with titles, main and featured artists, dates, genre, language, explicit flag and identifiers, as operations. | data | medium | kit-core, kit-ops |  |
| MUS-A6-F2-P2 | **Industry codes**: Takes the ISRC for each recording and the UPC or EAN for each release where the artist has them, and assigns ISRCs from a co-op registrant code where they do not (to be checked with the ISRC agencies). | service | small | MUS-A6-F2-P1 |  |
| MUS-A6-F2-P3 | **Explicit and content labels**: Explicit lyrics and other content labels set by the artist, used by listeners' filters and family settings. | data | small | MUS-A6-F2-P1 |  |
| MUS-A6-F3-P1 | **Cover art**: Square artwork checked for size, with versions for every screen, hidden metadata stripped (KIT-A15-F7-P3) and a check against known illegal images (KIT-A10-F10-P2). | integration | small | KIT-A15-F7-P3, KIT-A10-F10-P2, MUS-A6-F1-P1 |  |
| MUS-A6-F3-P2 | **Artwork description**: A text description of the artwork for screen readers, written or approved by the artist. | data | small | MUS-A6-F3-P1 |  |
| MUS-A6-F4-P1 | **Credits**: Roles from a published list that follows the music trade's DDEX role names, each linked to an artist page or given as a plain name. | data | medium | MUS-A6-F2-P1 |  |
| MUS-A6-F4-P2 | **Invite collaborators**: Collaborators get a link to confirm their credit and, where money is involved, their share (MUS-A7-F3-P1). | service | small | MUS-A6-F4-P1, MUS-A7-F3-P1 |  |
| MUS-A6-F5-P1 | **Release scheduler**: Releases go live at a set time in each listener's time zone or at one moment worldwide, on the kit's durable timers (KIT-A15-F14-P1). | integration | small | MUS-A6-F2-P1, KIT-A15-F14-P1 |  |
| MUS-A6-F5-P2 | **Pre-orders**: Buy before release; the download arrives on release day, with a refund if the release is cancelled. | service | small | MUS-A6-F5-P1, MUS-A9-F2-P1 |  |
| MUS-A6-F6-P1 | **Private preview links**: Streaming-only links that expire, on the kit's guest access by private link (KIT-A15-F8-P2), each logged with who it was sent to. | integration | small | KIT-A15-F8-P2, MUS-A16-F3-P2 |  |
| MUS-A6-F7-P1 | **Release versions**: Every change is a new version with history (KIT-A15-F3-P1); replacing audio keeps play counts only when the fingerprint shows it is the same recording. | service | small | MUS-A6-F2-P1, MUS-A11-F1-P2, KIT-A15-F3-P1 |  |
| MUS-A6-F7-P2 | **Take down your own music**: Artists take releases down at any time; buyers keep what they bought and are told. | service | small | MUS-A6-F2-P1, MUS-A3-F1-P1 |  |
| MUS-A6-F8-P1 | **Distributor feed**: Reads releases sent in DDEX's release format (ERN), which distributors use, under DDEX's free implementation licence once its terms for an open-source project are checked. | integration | large | MUS-A6-F2-P1, MUS-A6-F4-P1, MUS-A7-F1-P1 |  |
| MUS-A6-F8-P2 | **Bulk upload by command**: Upload many releases through the command API, with the same checks as the app. | service | small | MUS-A22-F1-P1, MUS-A6-F1-P1 |  |
| MUS-A6-F9-P1 | **Liner notes and booklets**: Notes and a PDF booklet with each release, shown accessibly with the kit's PDF reader (KIT-A27-F6-P1). | integration | small | MUS-A6-F2-P1, KIT-A27-F6-P1 |  |
| MUS-A6-F10-P1 | **Classical details**: Composer, work, movement, conductor and ensemble fields, so classical music can be found the way people look for it. | data | medium | MUS-A6-F2-P1, MUS-A6-F4-P1 |  |
| MUS-A6-F10-P2 | **Spoken word and podcasts**: Chapters, speed control and episode feeds, published as open podcast feeds. | service | medium | MUS-A6-F2-P1 |  |

## MUS-A7 Rights and splits

Every track says who owns the recording and who wrote the song, whether the writers belong to a collecting society, whether it is a cover, uses samples or is out of copyright, and how the money is split, with disagreements between collaborators handled fairly.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A7-F1** Declare your rights | Say on every upload that you own or control the recording and the song, or name who does. | core | P1 Rights declaration; P2 Rights step; P3 Artist agreement |
| **MUS-A7-F2** Songwriters and their societies | Name each songwriter and whether they belong to a collecting society, so their share reaches them. | core | P1 Songwriter records; P2 Song codes; P3 Writers outside any society |
| **MUS-A7-F3** Split the money | Agree who gets what share of each track's earnings, and have each person paid directly. | core | P1 Split sheets; P2 Split payouts; P3 Unconfirmed shares |
| **MUS-A7-F4** Covers | Release a cover version with the licence its country needs, and the original songwriters paid. | core | P1 Cover licensing; P2 Cover law per country |
| **MUS-A7-F5** Samples | Use a sample only with both owners' agreement, shown in the credits. | core | P1 Sample clearance; P2 Sample packs and loops |
| **MUS-A7-F6** Out of copyright and free licences | Release old works out of copyright, or music under a Creative Commons licence, labelled clearly. | core | P1 Public domain and Creative Commons; P2 Copyright terms per country |
| **MUS-A7-F7** Disagreements over rights | Raise a disagreement over credits, splits or ownership, with only that track's money held until it is settled. | core | P1 Rights disputes; P2 Panel decisions |
| **MUS-A7-F8** Publishers and labels' rights | Let a publisher or label manage the rights it holds for many artists, with statements for each. | next | P1 Publisher accounts |
| **MUS-A7-F9** DJ mixes and remixes | Release a DJ mix or a remix only with the rights for every track in it. | later | P1 Mixes and remixes |
| **MUS-A7-F10** Licence a track for film, games and adverts | Let someone license a track for a film, game or advert, at the rights owners' price. | later | P1 Sync licence requests |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A7-F1-P1 | **Rights declaration**: For each track: who owns the recording, who wrote the song and in what shares, whether it is original, a cover, uses samples or is out of copyright, and what licence covers each part. | data | medium | MUS-A6-F2-P1, MUS-A6-F4-P1 |  |
| MUS-A7-F1-P2 | **Rights step**: A plain-words step in the upload that asks each question once, says why it is asked, and remembers the answers for next time. | interface | medium | MUS-A7-F1-P1, MUS-A6-F1-P2 |  |
| MUS-A7-F1-P3 | **Artist agreement**: Short terms where the artist confirms the declaration is true, gives Spare Music only the rights it needs to stream and sell, and keeps everything else (KIT-A18-F4-P1). | legal | medium | KIT-A18-F4-P1 |  |
| MUS-A7-F2-P1 | **Songwriter records**: Songwriters with their collecting society, their writer number with it (the IPI number) and their publisher, if any. | data | small | MUS-A7-F1-P1 |  |
| MUS-A7-F2-P2 | **Song codes**: The ISWC code for each song where one exists, used to match songs in reports to societies. | data | small | MUS-A7-F2-P1 |  |
| MUS-A7-F2-P3 | **Writers outside any society**: A songwriter who belongs to no society is paid the songwriting share directly, on the same statement as the recording share. | service | small | MUS-A7-F2-P1, MUS-A7-F3-P2 |  |
| MUS-A7-F3-P1 | **Split sheets**: Shares of the recording and songwriting earnings per track, agreed by everyone named and changed only when all agree, kept as operations with history. | data | medium | MUS-A7-F1-P1, kit-ops |  |
| MUS-A7-F3-P2 | **Split payouts**: Each person on a split sheet is paid their share straight from the payout run (KIT-A3-F3-P2), so nobody in the middle holds other people's money. | service | medium | MUS-A7-F3-P1, KIT-A3-F3-P2 |  |
| MUS-A7-F3-P3 | **Unconfirmed shares**: A share whose owner has not joined is held for a published time, then handled as money that cannot be paid out under the kit's rules (KIT-A3-F3-P6). | service | small | MUS-A7-F3-P2, KIT-A3-F3-P6 |  |
| MUS-A7-F4-P1 | **Cover licensing**: A cover is allowed where Spare Music holds the country's licence for the song (MUS-A8-F1-P1) or the artist uploads proof of a licence; the songwriting share goes to the original writers. | service | medium | MUS-A7-F1-P1, MUS-A8-F1-P1 |  |
| MUS-A7-F4-P2 | **Cover law per country**: How covers are licensed for streaming and downloads in each country, such as the United States' compulsory licence for songs, run for digital services by The MLC, and society licences elsewhere. | research | medium |  |  |
| MUS-A7-F5-P1 | **Sample clearance**: A track with a sample names the sampled recording and song, with proof that the recording owner and the song owner agreed, since no country has a compulsory licence for samples (to be confirmed per country). | service | small | MUS-A7-F1-P1, MUS-A6-F4-P1 |  |
| MUS-A7-F5-P2 | **Sample packs and loops**: Loops and samples from packs are declared with the pack's name and licence, checked against its terms. | data | small | MUS-A7-F5-P1 |  |
| MUS-A7-F6-P1 | **Public domain and Creative Commons**: A song can be marked out of copyright (with the composer's dates) or under a Creative Commons licence, shown on the track, with the recording's own rights still declared. | data | small | MUS-A7-F1-P1 |  |
| MUS-A7-F6-P2 | **Copyright terms per country**: How long songs and recordings stay in copyright in each country, so out-of-copyright claims are checked against the right term. | research | small |  |  |
| MUS-A7-F7-P1 | **Rights disputes**: Disputes between collaborators or claimants on the kit's disputes service and evidence store (KIT-A11-F1-P1, KIT-A11-F2-P1), holding that track's earnings, never the artist's whole account. | integration | medium | KIT-A11-F1-P1, KIT-A11-F2-P1, MUS-A7-F3-P1 |  |
| MUS-A7-F7-P2 | **Panel decisions**: Unsettled disputes go to a member panel (KIT-A11-F4-P1), or outside to the courts with the evidence pack (KIT-A11-F7-P2). | integration | small | MUS-A7-F7-P1, KIT-A11-F4-P1, KIT-A11-F7-P2 |  |
| MUS-A7-F8-P1 | **Publisher accounts**: Publishers hold songwriting shares for the writers they represent, with their own statements, as kit organisations (KIT-A1-F7-P1). | integration | medium | MUS-A7-F2-P1, KIT-A1-F7-P1 |  |
| MUS-A7-F9-P1 | **Mixes and remixes**: A mix lists every track used with its codes, and is allowed only where licences or each owner's agreement cover all of them; the earnings are split by a published rule. | research | medium | MUS-A7-F1-P1, MUS-A11-F1-P2 |  |
| MUS-A7-F10-P1 | **Sync licence requests**: Requests to use a track in a video, film or game, with terms set by the rights owners and a short agreement signed online (KIT-A25-F1-P2). | service | medium | MUS-A7-F1-P1, KIT-A25-F1-P2 |  |

## MUS-A8 Collecting societies and licences per country

Each country gets its own rights pack: which bodies license songs and recordings there, which licences Spare Music holds, how plays and sales are reported and paid, which covers are allowed, and what changed. The songwriting share reaches writers through their societies, with every payment shown.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A8-F1** Licences held in each country | Know which licences Spare Music holds in each country, and what they allow. | core | P1 Rights packs per country; P2 Licensing research per country; P3 Licences page |
| **MUS-A8-F2** Songwriters paid through their societies | Have the songwriting share reach writers through their societies, with every payment shown. | core | P1 Usage reports; P2 Society payments; P3 Society reporting research; P4 Matching songs with societies |
| **MUS-A8-F3** Licence costs in the open | See what each licence costs and who carries it, as lines on the open cost ledger. | core | P1 Licence lines on the ledger; P2 Who carries a licence cost |
| **MUS-A8-F4** Only where licensed | Play and sell a song only in countries where it is licensed, and be told why if it is not. | core | P1 Where each track can play |
| **MUS-A8-F5** Label catalogues on at-cost terms | Let labels license whole catalogues to Spare Music on its at-cost terms. | next | P1 Standard label licence |
| **MUS-A8-F6** Told when the rules change | Be told when a country's rules for music change, and what it means for your music. | next | P1 Law and licence changes |
| **MUS-A8-F7** Radio-style stations | Listen to stations that play like radio, under the licences radio-style services use. | later | P1 Licences for radio-style play |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A8-F1-P1 | **Rights packs per country**: For each country: the bodies that license songs and recordings, the licences held, their terms, rates and reporting rules, in the format of the kit's country packs (KIT-A19-F6-P1). | data | medium | KIT-A19-F6-P1 |  |
| MUS-A8-F1-P2 | **Licensing research per country**: For each launch country: which licences an at-cost service for independent artists needs, what they cost, and how members' own songs are licensed where societies take an assignment of their rights (APRA AMCOS, PRS, SOCAN and GEMA do; the United States societies license without exclusivity). Needs a lawyer's view. | research | large |  |  |
| MUS-A8-F1-P3 | **Licences page**: A public page of the licences held in each country, their dates and their cost, beside the cost ledger. | interface | small | MUS-A8-F1-P1 |  |
| MUS-A8-F2-P1 | **Usage reports**: Reports of plays and sales per song to each society, in DDEX's sales report format (DSR) where the society takes it, checked against each society's rules. | service | large | MUS-A16-F5-P1, MUS-A16-F5-P3, MUS-A7-F2-P1, MUS-A7-F2-P2, MUS-A8-F1-P1 |  |
| MUS-A8-F2-P2 | **Society payments**: Pays each society what its licence asks for each period, shown on the artist's statement as the songwriting share sent through that society (KIT-A3-F3-P2). | service | medium | MUS-A8-F2-P1, KIT-A3-F3-P2 |  |
| MUS-A8-F2-P3 | **Society reporting research**: Each society's reporting format, periods, invoices and licences for small services, such as APRA AMCOS's Online Mini Licence and PRS for Music's licences for small online services. | research | medium |  |  |
| MUS-A8-F2-P4 | **Matching songs with societies**: Matches songs to each society's records by ISWC, writer numbers and titles, and lists unmatched songs for the writer to fix. | service | medium | MUS-A7-F2-P2, MUS-A8-F2-P1 |  |
| MUS-A8-F3-P1 | **Licence lines on the ledger**: What each society licence costs per period, how it is worked out and which plays it covers, as published lines on the open cost ledger (KIT-A13-F1-P2). | integration | small | KIT-A13-F1-P2, MUS-A8-F2-P2 |  |
| MUS-A8-F3-P2 | **Who carries a licence cost**: A published rule that a licence's cost is carried by the plays and sales of the songs it covers, not by artists who own all their rights and belong to no society. | legal | medium | MUS-A8-F3-P1, MUS-A8-F1-P2 |  |
| MUS-A8-F4-P1 | **Where each track can play**: Each track plays and sells only where its rights allow, from the rights declaration and the country's pack; listeners see a plain reason when something is not available. | service | small | MUS-A7-F1-P1, MUS-A8-F1-P1 |  |
| MUS-A8-F5-P1 | **Standard label licence**: One at-cost licence a label can sign for its catalogue, with no advances or minimum payments, published in full. | legal | medium | MUS-A7-F1-P3, MUS-A7-F8-P1 |  |
| MUS-A8-F6-P1 | **Law and licence changes**: Rights packs record each change with its date, and the artists and listeners affected are told through co-op announcements (KIT-A8-F8-P1). | service | small | MUS-A8-F1-P1, KIT-A8-F8-P1 |  |
| MUS-A8-F7-P1 | **Licences for radio-style play**: Which licences cover radio-style play in each country, such as SoundExchange's for non-interactive streams in the United States, if members want stations. | research | medium |  |  |

## MUS-A9 Paying for music

Each artist chooses how their music is sold: free, buy to keep, pay what you like, part of a listening pass if members choose one, or stream to own. Listeners see the price, the running costs and what reaches the artist before they pay, and can support artists directly.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A9-F1** Artists choose how they sell | Set for each release whether it is free, buy to keep, pay what you like, in the listening pass or stream to own. | core | P1 Pricing choices; P2 Pricing step |
| **MUS-A9-F2** Buy music to keep | Buy a track or album and download it in the format you want, yours to keep with no lock. | core | P1 Buy to keep; P2 Download formats; P3 Pay what you like |
| **MUS-A9-F3** See where the money goes | Before paying, see the running costs and what the artist gets. | core | P1 At-cost fee on music; P2 Money breakdown |
| **MUS-A9-F4** Support an artist directly | Tip an artist or support them monthly, with all of it reaching them. | core | P1 Tips; P2 Monthly support; P3 Fair support terms |
| **MUS-A9-F5** The listening pass | Pay one monthly amount to stream everything in the pass, if members choose to offer one. | core | P1 Pass decision; P2 Listening pass; P3 How the pass is shared out; P4 Where your pass went; P5 Pass price rule |
| **MUS-A9-F6** Free listening | Listen to free releases and previews at no charge, with their running costs covered in the open. | core | P1 Free streams; P2 Who covers free listening |
| **MUS-A9-F7** Pay on any device | Buy and pay in a way each app store allows, so artists keep everything except running costs on phones too. | core | P1 App store rules for music; P2 Purchase plan per store |
| **MUS-A9-F8** Refunds | Get a refund on a mistaken purchase or a broken file, by published rules. | core | P1 Refunds and disputes |
| **MUS-A9-F9** Stream to own | Pay a little for each play until you own the track, if the artist chooses it. | next | P1 Stream to own; P2 Progress to owning |
| **MUS-A9-F10** Discography bundles | Buy an artist's whole catalogue, or several releases at once, at a price the artist sets. | next | P1 Bundles |
| **MUS-A9-F11** Download codes | Hand out codes at shows or with records that unlock a download. | next | P1 Download codes |
| **MUS-A9-F12** Gifts | Give an album or a month of support to someone else. | next | P1 Gift a release |
| **MUS-A9-F13** Records, CDs and merch | Buy records, CDs and merch from the artist, with the download included. | later | P1 Physical sales |
| **MUS-A9-F14** Prices in your own money | See prices in your own currency, with exchange costs shown. | later | P1 Local currency prices |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A9-F1-P1 | **Pricing choices**: The ways a release can be sold, with prices in the artist's currency (KIT-A3-F1-P4) and the lowest pay-what-you-like price, set per release and per track. | data | medium | MUS-A6-F2-P1, KIT-A3-F1-P4 |  |
| MUS-A9-F1-P2 | **Pricing step**: A step in the upload that shows, for each choice, what a listener pays, the running costs and what reaches the artist. | interface | small | MUS-A9-F1-P1, MUS-A9-F3-P1 |  |
| MUS-A9-F2-P1 | **Buy to keep**: Checkout on the kit's payments (KIT-A3-F1-P3) with a receipt (KIT-A3-F7-P1), adding the release to the buyer's library for good. | integration | medium | KIT-A3-F1-P3, KIT-A3-F7-P1, MUS-A9-F1-P1 |  |
| MUS-A9-F2-P2 | **Download formats**: Downloads in FLAC, Opus, MP3 or other formats the codec study clears (KIT-A27-F8-P1), with tags, artwork and lyrics inside. | service | medium | MUS-A16-F3-P1, KIT-A27-F8-P1 |  |
| MUS-A9-F2-P3 | **Pay what you like**: Pay more than the lowest price, or nothing for a free release, with what the artist receives shown as you type. | interface | small | MUS-A9-F2-P1 |  |
| MUS-A9-F3-P1 | **At-cost fee on music**: Works out the running cost of each sale and stream from the open cost ledger, with the kit's fee calculator and fee breakdown (KIT-A3-F4-P1, KIT-A3-F4-P2); nothing is added on top. | integration | small | KIT-A3-F4-P1, KIT-A3-F4-P2, svc-ledger |  |
| MUS-A9-F3-P2 | **Money breakdown**: Every price shows the card fee, delivery and storage, any songwriting share sent to a society, any tax, and what the artist gets. | interface | small | MUS-A9-F3-P1 |  |
| MUS-A9-F4-P1 | **Tips**: Tips on the kit's tips (KIT-A3-F6-P1): the whole tip reaches the artist, and the card cost is carried as a running cost on the ledger. | integration | small | KIT-A3-F6-P1 |  |
| MUS-A9-F4-P2 | **Monthly support**: Monthly support at amounts the artist sets, on the kit's regular payments (KIT-A3-F12-P1), with extras such as early releases and supporter-only tracks. | integration | medium | KIT-A3-F12-P1, MUS-A1-F4-P1 |  |
| MUS-A9-F4-P3 | **Fair support terms**: Cancel in one step, notice before any price change, and the refunds consumer law gives in each country (KIT-A18-F8-P1). | service | small | MUS-A9-F4-P2, KIT-A18-F8-P1 |  |
| MUS-A9-F5-P1 | **Pass decision**: A decision note comparing no pass, a pass shared by each listener's own plays and a pooled pass, on what artists earn, fake-play risk, licence cost and running cost, for the members' vote. | research | medium |  |  |
| MUS-A9-F5-P2 | **Listening pass**: A monthly pass on the kit's regular payments (KIT-A3-F12-P1), with an optional monthly cap (KIT-A3-F12-P2); its price is running costs plus the amount members vote goes to artists. | integration | medium | KIT-A3-F12-P1, KIT-A3-F12-P2, MUS-A9-F5-P1 |  |
| MUS-A9-F5-P3 | **How the pass is shared out**: Shares each listener's pass among the artists that listener played, or pools every pass by share of all plays, whichever members choose; both are built so the vote can pick. | library | medium | MUS-A9-F5-P2, MUS-A16-F5-P1, MUS-A11-F5-P1 |  |
| MUS-A9-F5-P4 | **Where your pass went**: Each month, see which artists your pass paid and how much. | interface | small | MUS-A9-F5-P3 |  |
| MUS-A9-F5-P5 | **Pass price rule**: How the pass price is set from running costs and the amount members vote goes to artists, published with every change. | legal | small | MUS-A9-F5-P2, MUS-A21-F1-P1 |  |
| MUS-A9-F6-P1 | **Free streams**: Free releases and previews play for everyone; their delivery cost is a ledger line covered by a rule members vote on, such as a free allowance (KIT-A13-F3-P7). | service | small | MUS-A16-F3-P2, KIT-A13-F3-P7 |  |
| MUS-A9-F6-P2 | **Who covers free listening**: A published rule, set by vote, for how the delivery cost of free listening is shared, such as a small part of every sale's running cost. | legal | small | MUS-A9-F6-P1, MUS-A21-F1-P1 |  |
| MUS-A9-F7-P1 | **App store rules for music**: When Apple and Google require their own payment, and their cut, for music sales, passes and tips, and where linking to the web is allowed, following the kit's app store rules (KIT-A20-F8-P1). | legal | medium | KIT-A20-F8-P1 | [K04](../kit/STATUS.md) |
| MUS-A9-F7-P2 | **Purchase plan per store**: Buy on the web, link out where allowed, or sign in to music bought elsewhere, so a store's cut is never hidden in an artist's price. | service | medium | MUS-A9-F7-P1, MUS-A9-F2-P1 |  |
| MUS-A9-F8-P1 | **Refunds and disputes**: Refunds, card disputes and who carries a loss on the kit's rules (KIT-A3-F5-P1, KIT-A3-F5-P2, KIT-A3-F16-P1), with a published rule for music already downloaded. | integration | small | KIT-A3-F5-P1, KIT-A3-F5-P2, KIT-A3-F16-P1 |  |
| MUS-A9-F9-P1 | **Stream to own**: Each play costs a little more until the listener has paid the track's price and owns it, billed monthly after use so the co-op never holds a prepaid balance (KIT-A3-F12-P2); the method the Resonate co-op used until it closed in 2024. | service | medium | MUS-A9-F1-P1, MUS-A16-F5-P1, KIT-A3-F12-P2 |  |
| MUS-A9-F9-P2 | **Progress to owning**: Each track shows how close you are to owning it and what you have paid. | interface | small | MUS-A9-F9-P1 |  |
| MUS-A9-F10-P1 | **Bundles**: Bundles of releases with their own price set by the artist, with what each release in the bundle earns shown on the statement. | service | small | MUS-A9-F1-P1, MUS-A9-F2-P1 |  |
| MUS-A9-F11-P1 | **Download codes**: Single-use codes an artist makes for a release, printable as cards, each redeemed once and logged. | service | small | MUS-A9-F2-P1, MUS-A3-F1-P1 |  |
| MUS-A9-F12-P1 | **Gift a release**: Buy a release or support for someone, who gets a link to claim it. | service | small | MUS-A9-F2-P1 |  |
| MUS-A9-F13-P1 | **Physical sales**: Physical items on the kit's marketplace engine (KIT-A5-F1-P1) with stock (KIT-A5-F10-P1), sent by the artist, unlocking the digital album when bought. | integration | medium | KIT-A5-F1-P1, KIT-A5-F10-P1, kit-market, MUS-A9-F2-P1 |  |
| MUS-A9-F14-P1 | **Local currency prices**: Prices in the listener's currency on the kit's currencies per country (KIT-A3-F14-P1), with the exchange cost published. | integration | small | KIT-A3-F14-P1, MUS-A9-F1-P1 |  |

## MUS-A10 Earnings, payouts and tax

Artists see what every play and sale earned, with the running costs and any society share shown, get paid on a regular schedule, have tax charged and reported as each country requires, and are protected from fraud.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A10-F1** See what you earned | See earnings per track, release and country, every play and sale accounted for. | core | P1 Earnings ledger; P2 Earnings page |
| **MUS-A10-F2** Get paid out | Set up payouts once and get paid on a regular schedule. | core | P1 Payout set-up; P2 Payout runs; P3 No play minimum |
| **MUS-A10-F3** Statements | Download monthly statements that show every line, ready for an accountant. | core | P1 Statements |
| **MUS-A10-F4** Tax on sales in each country | Know the right tax is charged on every sale and passed on, wherever the buyer is. | core | P1 Tax research for music; P2 Tax on music sales; P3 Income reporting |
| **MUS-A10-F5** Safe from fraud and money laundering | Know payouts are protected from stolen cards, card testing and money laundering. | core | P1 Payment fraud checks; P2 Money laundering and sanctions; P3 First payout holds |
| **MUS-A10-F6** Tax held back for you | Have tax held back from payouts if you choose. | next | P1 Withholding at payout |
| **MUS-A10-F7** Pay session players from earnings | Pay a session player or producer a set fee from future earnings. | next | P1 Fees from earnings |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A10-F1-P1 | **Earnings ledger**: Every sale, pass share and tip, with the running cost and any society share beside it, adding up to the cent (KIT-A3-F9-P1). | service | medium | MUS-A16-F5-P3, MUS-A16-F5-P1, KIT-A3-F9-P1, MUS-A9-F3-P1 |  |
| MUS-A10-F1-P2 | **Earnings page**: Earnings by day, month, track and country, with running costs and society shares shown, on the kit's earnings page (KIT-A3-F3-P3). | interface | medium | MUS-A10-F1-P1, KIT-A3-F3-P3 |  |
| MUS-A10-F2-P1 | **Payout set-up**: Payout details on the kit's payouts (KIT-A3-F3-P1), with the identity checks the provider needs (KIT-A2-F1-P2). | integration | small | KIT-A3-F3-P1, KIT-A2-F1-P2 | [K02](../kit/STATUS.md) |
| MUS-A10-F2-P2 | **Payout runs**: Regular payouts through the kit's payout runs (KIT-A3-F3-P2), with failed payouts retried (KIT-A3-F3-P5). | integration | small | KIT-A3-F3-P2, KIT-A3-F3-P5, MUS-A10-F2-P1 |  |
| MUS-A10-F2-P3 | **No play minimum**: Every play earns from the first one; small balances are carried forward until they pass the cost of paying them out, a rule shown on the statement. | legal | small | MUS-A10-F2-P2 |  |
| MUS-A10-F3-P1 | **Statements**: Monthly and yearly statements in open formats from the kit's statements (KIT-A4-F4-P1). | integration | small | KIT-A4-F4-P1, MUS-A10-F1-P1 |  |
| MUS-A10-F4-P1 | **Tax research for music**: For each launch country: who the seller is for downloads, passes and tips (KIT-A3-F1-P5), when a platform must charge GST or VAT on digital services sold through it, and how artists' income is reported. Needs a tax adviser's view. | research | medium | KIT-A3-F1-P5 |  |
| MUS-A10-F4-P2 | **Tax on music sales**: Works out and collects GST or VAT where the co-op counts as the seller of digital music, on the kit's tax engine (KIT-A4-F1-P2) and country packs (KIT-A19-F6-P1); the kit's tax rules per country (KIT-A4-F7) are marked later and must move up before sales open in a second country. | integration | large | KIT-A4-F1-P2, KIT-A19-F6-P1, MUS-A10-F4-P1 |  |
| MUS-A10-F4-P3 | **Income reporting**: Reports artists' income where platform reporting rules require it, with the details collected once (KIT-A4-F3-P1, KIT-A4-F3-P2). | integration | medium | KIT-A4-F3-P1, KIT-A4-F3-P2 |  |
| MUS-A10-F5-P1 | **Payment fraud checks**: Card testing, stolen cards used to buy one's own music, and tips used to move money, caught with the kit's fraud screening and published risk rules (KIT-A3-F13-P1, KIT-A3-F13-P2). | integration | small | KIT-A3-F13-P1, KIT-A3-F13-P2 |  |
| MUS-A10-F5-P2 | **Money laundering and sanctions**: Whether money-laundering rules cover music sales and payouts in each country, under the kit's programme (KIT-A4-F5-P1), with sanctions screening of payouts (KIT-A4-F8-P1). | legal | small | KIT-A4-F5-P1, KIT-A4-F8-P1 | [K02](../kit/STATUS.md) |
| MUS-A10-F5-P3 | **First payout holds**: A new artist's first payouts are held for a published time, and negative balances after chargebacks are recovered within the kit's limits (KIT-A3-F16-P2). | integration | small | KIT-A3-F16-P2, MUS-A10-F2-P2 |  |
| MUS-A10-F6-P1 | **Withholding at payout**: Uses the kit's withholding at payout (KIT-A4-F12-P2) for artists who want it. | integration | small | KIT-A4-F12-P2 |  |
| MUS-A10-F7-P1 | **Fees from earnings**: A set fee to a collaborator paid from future earnings of a track, shown on both statements until it is paid. | service | small | MUS-A7-F3-P2 |  |

## MUS-A11 Stopping copies and stolen uploads

Uploads are fingerprinted and compared with the catalogue so nobody can upload someone else's music and take its earnings. A match goes to a person, never an automatic strike. Copyright notices are handled on time, impersonators are stopped, fake plays are found and left out, and copy protection follows the members' decision.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A11-F1** Stolen uploads caught | Know that someone uploading your music as theirs is caught before it earns. | core | P1 Fingerprint method study; P2 Fingerprint index; P3 Upload check; P4 Matches reviewed by people |
| **MUS-A11-F2** Protect your catalogue | Add your recordings to the reference list so copies are caught, including ones uploaded before you joined. | core | P1 Reference recordings; P2 Check what is already up |
| **MUS-A11-F3** Copyright notices | Send a copyright notice about a track, answer one with a counter-notice, and see the outcome. | core | P1 Notice and counter-notice; P2 Money held during a claim; P3 Repeat infringer policy; P4 Host duties per country |
| **MUS-A11-F4** Stop impersonators | Know nobody can release music as you or put a fake release on your page. | core | P1 Impersonation checks; P2 Releases on the wrong page |
| **MUS-A11-F5** Fake plays left out | Know plays from bots and play farms are found, left out of charts and payouts, and logged. | core | P1 Fake play detection; P2 Fake play research; P3 What happens to fake plays |
| **MUS-A11-F6** Copy protection decision | Know how streamed music is protected, decided by members with the trade-offs shown. | next | P1 Copy protection decision note; P2 Offline copies kept in the app |
| **MUS-A11-F7** Watermarks in each copy | Have each download or private preview carry a hidden mark, so a leaked copy leads back to its account, if members choose it. | next | P1 Watermark study; P2 Per-copy watermark |
| **MUS-A11-F8** Find copies elsewhere | Find your music uploaded on other sites, if you choose, so you can send notices yourself. | later | P1 Copies on other sites |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A11-F1-P1 | **Fingerprint method study**: Compares open audio fingerprinting methods, such as Chromaprint (LGPL-2.1 as a whole) and landmark methods like audfprint (MIT), on accuracy with noise and edits, speed, licence and patents (the first landmark fingerprint patent, US 6,990,453, has expired; later patents need a search), before any code is chosen. | research | medium |  |  |
| MUS-A11-F1-P2 | **Fingerprint index**: Fingerprints every uploaded recording and keeps a searchable index of the whole catalogue. | library | large | MUS-A11-F1-P1, MUS-A16-F2-P1 |  |
| MUS-A11-F1-P3 | **Upload check**: Each new upload is compared with the index; a close match to someone else's recording holds the release for review before it goes live. | service | medium | MUS-A11-F1-P2, MUS-A6-F1-P1 |  |
| MUS-A11-F1-P4 | **Matches reviewed by people**: A trained moderator compares a held match with both rights declarations; the uploader can show a licence, and nothing becomes a strike without a person's decision. | service | medium | MUS-A11-F1-P3, MUS-A14-F7-P2 |  |
| MUS-A11-F2-P1 | **Reference recordings**: Verified artists and labels add reference fingerprints for their recordings, including music not released on Spare Music, without uploading the audio. | service | small | MUS-A11-F1-P2, MUS-A1-F5-P1 |  |
| MUS-A11-F2-P2 | **Check what is already up**: New references are compared with the existing catalogue, and matches go to review. | service | small | MUS-A11-F2-P1, MUS-A11-F1-P4 |  |
| MUS-A11-F3-P1 | **Notice and counter-notice**: Notice and counter-notice forms on the kit's notices and deadlines (KIT-A10-F21-P2, KIT-A10-F9-P2), meeting each country's rules for hosts (KIT-A10-F21-P1), usable without an account (KIT-A10-F3-P3). | integration | medium | KIT-A10-F21-P1, KIT-A10-F21-P2, KIT-A10-F9-P2, KIT-A10-F3-P3 |  |
| MUS-A11-F3-P2 | **Money held during a claim**: While a claim is open, that track's earnings are held, then paid to whoever the outcome favours, never kept by the co-op. | service | small | MUS-A11-F3-P1, MUS-A10-F1-P1 |  |
| MUS-A11-F3-P3 | **Repeat infringer policy**: A published policy for accounts with repeated upheld copyright notices, on the enforcement ladder. | legal | small | MUS-A14-F2-P1, MUS-A14-F6-P1 |  |
| MUS-A11-F3-P4 | **Host duties per country**: What each country asks of a music host: notice and takedown, Canada's notice and notice, Australia's safe harbour that does not cover commercial platforms (to be checked), and the EU's duties for content-sharing services to try to get licences and to keep notified works off. | research | large |  |  |
| MUS-A11-F4-P1 | **Impersonation checks**: Releases credited to a verified artist need that artist's approval; look-alike names and artwork are flagged for review. | service | small | MUS-A1-F5-P1, MUS-A6-F2-P1 |  |
| MUS-A11-F4-P2 | **Releases on the wrong page**: Artists report a release put on their page by someone else, and it comes off the page while it is reviewed. | service | small | MUS-A11-F4-P1, MUS-A14-F1-P1 |  |
| MUS-A11-F5-P1 | **Fake play detection**: Published signals such as new accounts playing in loops, repeated short plays and play farms, with the details that would help fraudsters kept back (svc-trust). | service | large | MUS-A16-F5-P1, svc-trust, MUS-A11-F5-P2 |  |
| MUS-A11-F5-P2 | **Fake play research**: Proven ways to find artificial plays, from published research and the rules other services publish, and what can be shown without helping fraudsters. | research | medium |  |  |
| MUS-A11-F5-P3 | **What happens to fake plays**: Fake plays earn nothing and count for nothing; the money stays with the listener's share or the pool, never the co-op, and repeat offenders follow the ladder. | legal | small | MUS-A11-F5-P1, MUS-A14-F6-P1 |  |
| MUS-A11-F6-P1 | **Copy protection decision note**: Compares closed playback locks (DRM, each needing an agreement with the company that owns it), a hidden watermark in each copy, and no lock, on what each stops, cost, licence and fairness to listeners. | research | medium |  |  |
| MUS-A11-F6-P2 | **Offline copies kept in the app**: Streamed music saved for offline is kept encrypted in the app's own storage, if the decision picks it; bought music stays the buyer's to copy. | library | medium | MUS-A11-F6-P1, MUS-A2-F6-P1 |  |
| MUS-A11-F7-P1 | **Watermark study**: Compares open audio watermarking methods, such as AudioSeal (MIT), on how well marks survive re-recording and compression, sound quality, licence and patents. | research | medium |  |  |
| MUS-A11-F7-P2 | **Per-copy watermark**: A hidden mark in each copy, if the decision picks it, using an openly licensed method cleared for patents. | library | large | MUS-A11-F7-P1, MUS-A11-F6-P1, MUS-A16-F3-P1 |  |
| MUS-A11-F8-P1 | **Copies on other sites**: Looks for a recording's fingerprint on sites whose terms allow it, and gives the artist the evidence to send their own notice. | research | medium | MUS-A11-F1-P2 |  |

## MUS-A12 Artists and fans together

Following artists, artist posts and news, shows and tour dates, supporter extras, a mailing list the artist owns, sharing music anywhere with no tracking, an artist's own site, and messages between artists and fans.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A12-F1** Follow artists | Follow an artist and hear about every release, show and post, in time order. | core | P1 Follow; P2 Release alerts |
| **MUS-A12-F2** Artist posts | Read news, photos and notes from artists you follow, and reply where they allow it. | core | P1 Artist posts |
| **MUS-A12-F3** A mailing list the artist owns | Email fans and buyers who agreed to hear from you, and take the list with you. | core | P1 Fan mailing list |
| **MUS-A12-F4** Share music anywhere | Share a track, album or playlist as a link that plays in any browser, with no tracking. | core | P1 Share links and cards; P2 Embed player |
| **MUS-A12-F5** Shows and tour dates | See an artist's shows near you, with a link to buy tickets. | next | P1 Show listings |
| **MUS-A12-F6** Supporter extras | Get early releases, demos and supporter-only posts from artists you support. | next | P1 Supporter-only music and posts |
| **MUS-A12-F7** Your own artist site | Run a simple site on your own web address with your music, shows and shop. | next | P1 Artist site |
| **MUS-A12-F8** Messages and listening parties | Message an artist who allows it, and join an artist's listening party for a new release. | later | P1 Artist messages; P2 Listening parties |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A12-F1-P1 | **Follow**: Follow artists and labels; who you follow is private unless you show it. | service | small | MUS-A1-F4-P1 |  |
| MUS-A12-F1-P2 | **Release alerts**: New releases and shows from followed artists through the kit's notifications, as the listener chooses (KIT-A8-F3-P2, KIT-A8-F5-P1). | integration | small | KIT-A8-F3-P2, KIT-A8-F5-P1, MUS-A12-F1-P1 |  |
| MUS-A12-F2-P1 | **Artist posts**: Short posts with photos and audio clips on the artist page and in followers' release feed, with comments if the artist turns them on (KIT-A8-F12-P1). | service | medium | MUS-A1-F4-P1, KIT-A8-F12-P1 |  |
| MUS-A12-F3-P1 | **Fan mailing list**: Buyers and followers who opt in, kept by the artist and exportable any time, sent through the kit's email with consent records (KIT-A8-F4-P1, KIT-A8-F5-P2). | integration | medium | KIT-A8-F4-P1, KIT-A8-F5-P2, MUS-A12-F1-P1 |  |
| MUS-A12-F4-P1 | **Share links and cards**: Links and preview cards (KIT-A28-F2-P3) that open the web player, without tracking who opened them. | integration | small | KIT-A28-F2-P3, MUS-A16-F3-P3 |  |
| MUS-A12-F4-P2 | **Embed player**: A small player artists can put on their own site, through the oEmbed standard, with no tracking. | interface | small | MUS-A16-F3-P3 |  |
| MUS-A12-F5-P1 | **Show listings**: Shows with date, place and ticket link, with places from the kit's address search (KIT-A7-F2-P2). | service | small | MUS-A1-F4-P1, KIT-A7-F2-P2 |  |
| MUS-A12-F6-P1 | **Supporter-only music and posts**: Releases and posts only an artist's supporters can play or read, delivered with signed links. | service | small | MUS-A9-F4-P2, MUS-A12-F2-P1 |  |
| MUS-A12-F7-P1 | **Artist site**: An artist site on the kit's own-site builder and custom domains (KIT-A28-F4-P1, KIT-A28-F1-P2), with the embed player. | integration | medium | KIT-A28-F4-P1, KIT-A28-F1-P2, MUS-A12-F4-P2 |  |
| MUS-A12-F8-P1 | **Artist messages**: Messages on the kit's chat service (KIT-A8-F1-P2), with artists choosing who can write to them. | integration | small | KIT-A8-F1-P2 |  |
| MUS-A12-F8-P2 | **Listening parties**: An artist plays a new release to fans at one time, with chat, in a shared listening room. | service | small | MUS-A2-F9-P1 |  |

## MUS-A13 Artist studio and stats

The artist's own workspace: a dashboard, stats that protect listeners' privacy, every release in one place, counted promotion links, a release planner, and where your music is being played and added.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A13-F1** See how your music is doing | See plays, listeners, buyers, supporters and saves for each track, by day and country. | core | P1 Stats; P2 Stats page |
| **MUS-A13-F2** Dashboard | See earnings, plays, new supporters, payouts and anything that needs you, on one page. | core | P1 Artist dashboard |
| **MUS-A13-F3** Know your supporters | See who bought your music or supports you, where they chose to share their name, and thank them. | core | P1 Supporters list; P2 Thank-you notes |
| **MUS-A13-F4** Manage every release | See every release, draft and scheduled date, and edit any of them. | core | P1 Release manager |
| **MUS-A13-F5** Counted promotion links | Make a link for a release that shows how many visits and sales it brought, without tracking anyone. | next | P1 Counted links |
| **MUS-A13-F6** Plan a release | Plan a release with a checklist and dates. | next | P1 Release planner |
| **MUS-A13-F7** Who adds your music | See which public playlists include your tracks. | next | P1 Playlist reach |
| **MUS-A13-F8** Import from music tools | Turn a mixed and mastered track from any music tool into a draft release. | later | P1 Draft from exported files |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A13-F1-P1 | **Stats**: Counts per track and release by day and country, from plays that passed the fake-play checks, with small numbers hidden so no single listener can be picked out (KIT-A13-F9-P2). | service | medium | MUS-A16-F5-P1, MUS-A11-F5-P1, KIT-A13-F9-P2 |  |
| MUS-A13-F1-P2 | **Stats page**: Charts and tables in the dashboard on the kit's chart engine (KIT-A27-F3-P3), downloadable as CSV. | interface | small | MUS-A13-F1-P1, KIT-A27-F3-P3 |  |
| MUS-A13-F2-P1 | **Artist dashboard**: One page for earnings, plays, held uploads, unconfirmed splits and to-dos, on the kit's seller to-do page (KIT-A5-F23-P2). | interface | medium | MUS-A10-F1-P1, MUS-A13-F1-P1, KIT-A5-F23-P2 |  |
| MUS-A13-F3-P1 | **Supporters list**: Buyers and supporters who agreed to share their name, with what they bought, never their contact details unless they joined the mailing list. | service | small | MUS-A16-F5-P3, MUS-A9-F4-P2 |  |
| MUS-A13-F3-P2 | **Thank-you notes**: Send a thank-you note to a buyer or supporter, delivered by the kit's messaging (svc-messaging). | interface | small | MUS-A13-F3-P1, svc-messaging |  |
| MUS-A13-F4-P1 | **Release manager**: Releases and tracks with their state (draft, held, scheduled, live, taken down) and edits for prices and countries in bulk. | interface | medium | MUS-A6-F2-P1, MUS-A9-F1-P1 |  |
| MUS-A13-F5-P1 | **Counted links**: Links that count the visits and sales they lead to, with the kit's private usage counts (KIT-A22-F8-P1), no cookies and no tracking of people. | service | small | MUS-A12-F4-P1, KIT-A22-F8-P1 |  |
| MUS-A13-F6-P1 | **Release planner**: Steps from master to release day: artwork, credits, splits, rights, previews and pre-orders, with reminders. | interface | small | MUS-A6-F5-P1 |  |
| MUS-A13-F7-P1 | **Playlist reach**: Public playlists that include a track, with the maker's name. | service | small | MUS-A3-F2-P1 |  |
| MUS-A13-F8-P1 | **Draft from exported files**: A draft release made from files any audio tool exports, through the command API. | integration | small | MUS-A22-F1-P1, MUS-A6-F1-P1 |  |

## MUS-A14 Moderation, legal duties and transparency

Removing only what the law requires, by published rules, in the open: reports from anyone, a rule book linked to each country's law, fast removal of illegal material, a public removal log, statements of reasons and appeals, a published ladder before any ban, elected moderators, spam defences, and police and government requests checked and counted.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A14-F1** Report anything | Report a track, artwork, lyrics, playlist, page or post against a named rule, and hear what happened. | core | P1 Report flow; P2 Reporter updates |
| **MUS-A14-F2** Read the rules | Read the rules in plain words, each linked to the law that requires it. | core | P1 Rule book; P2 Rules mapped to each country's law |
| **MUS-A14-F3** Illegal material removed fast | Know illegal material, such as terrorist recordings, child abuse images in artwork, credible threats and music a court orders down, is removed fast. | core | P1 Known material matching; P2 Legal deadline clock; P3 Country-only restriction; P4 Hate lyrics and incitement |
| **MUS-A14-F4** See every removal in public | Browse a public log of every removal, held upload, section move and copyright decision, with the rule and the law behind it. | core | P1 Public removal log; P2 Transparency reports |
| **MUS-A14-F5** Appeal any decision | Appeal a removal, held upload, section move or strike, first to another moderator, then to a member panel. | core | P1 Appeals; P2 Statement of reasons |
| **MUS-A14-F6** Know the steps before a ban | See a published ladder of warning, suspension and ban, applied the same way to everyone. | core | P1 Enforcement ladder; P2 Strikes and restoring |
| **MUS-A14-F7** Elected moderators | Vote for moderators with fixed terms, and see the tools and limits they work under. | core | P1 Moderator elections; P2 Moderator workbench; P3 Moderator training and wellbeing |
| **MUS-A14-F8** Stop spam and fake artists | Keep the catalogue free of spam uploads, noise tracks and networks of fake artists. | core | P1 Spam uploads; P2 Noise and spam rules |
| **MUS-A14-F9** Police and government requests | Know any request for your data or to remove music is checked against the law and counted in public. | core | P1 Request desk |
| **MUS-A14-F10** Trusted flaggers and second looks | Know reports from recognised bodies are seen first, and a sample of decisions is checked again. | next | P1 Second review; P2 Trusted flaggers |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A14-F1-P1 | **Report flow**: A short flow on every item on the kit's reports (KIT-A10-F3-P1, KIT-A10-F3-P2): pick the rule, add detail, block too if you want. | integration | medium | svc-trust, KIT-A10-F3-P1, KIT-A10-F3-P2 |  |
| MUS-A14-F1-P2 | **Reporter updates**: Tells the reporter what was decided, why, and how to appeal. | service | small | MUS-A14-F1-P1, svc-messaging |  |
| MUS-A14-F2-P1 | **Rule book**: Versioned plain-language rules on the kit's rules library (KIT-A10-F5-P1), changed only by vote; the promise to remove only what the law requires is protected (MUS-A21-F1-P3). | data | small | KIT-A10-F5-P1, svc-voting, MUS-A21-F1-P3 |  |
| MUS-A14-F2-P2 | **Rules mapped to each country's law**: For each rule, the law in each country that requires it, kept with the rights packs and country packs. | research | medium | MUS-A14-F2-P1 |  |
| MUS-A14-F3-P1 | **Known material matching**: Artwork and images checked with the kit's known-content matching (KIT-A10-F10-P2), and audio checked against lists from authorised bodies where such lists exist. | integration | medium | KIT-A10-F10-P2, MUS-A16-F2-P1 |  |
| MUS-A14-F3-P2 | **Legal deadline clock**: A clock for each kind of notice, with on-call cover and alerts before a deadline is missed, on the kit's legal notice handling (KIT-A10-F9-P2) and urgent route (KIT-A21-F3-P1). | integration | small | KIT-A10-F9-P2, KIT-A21-F3-P1 |  |
| MUS-A14-F3-P3 | **Country-only restriction**: A track that is unlawful in one country is hidden only there, if the demand passes a published test, and the restriction is logged in public. | service | small | MUS-A8-F4-P1, MUS-A14-F9-P1 |  |
| MUS-A14-F3-P4 | **Hate lyrics and incitement**: When lyrics cross into illegal hate speech or incitement in each country, so only what the law forbids comes down, and lawful but offensive music stays up with a label if one fits. | research | medium |  |  |
| MUS-A14-F4-P1 | **Public removal log**: Spare Music's actions in the kit's public removal log (KIT-A10-F7-P1): every takedown, held upload, section move and copyright decision, with its rule and law, never exposing victims. | integration | medium | KIT-A10-F7-P1, MUS-A14-F1-P1 |  |
| MUS-A14-F4-P2 | **Transparency reports**: Regular counts per rule, matches reviewed, notices received and upheld, and government requests, as open data (KIT-A10-F15-P1). | integration | small | KIT-A10-F15-P1, MUS-A14-F4-P1 |  |
| MUS-A14-F5-P1 | **Appeals**: Two-step appeals with deadlines on the kit's appeals and panels (KIT-A11-F5-P1, KIT-A11-F4-P1), with the outcome logged. | integration | medium | KIT-A11-F5-P1, KIT-A11-F4-P1, MUS-A14-F1-P1 |  |
| MUS-A14-F5-P2 | **Statement of reasons**: The artist or listener acted on is told the rule, the reason and how to appeal (KIT-A10-F8-P1). | integration | small | KIT-A10-F8-P1 |  |
| MUS-A14-F6-P1 | **Enforcement ladder**: Published steps for repeated breaches, such as uploading other people's music or false AI-use answers, with when each step expires and how an account is restored. | legal | small | MUS-A14-F2-P1 |  |
| MUS-A14-F6-P2 | **Strikes and restoring**: Records each upheld breach as a strike that expires, applies the next step, and restores the account when a ban ends or an appeal wins. | service | small | MUS-A14-F6-P1, MUS-A14-F1-P1 |  |
| MUS-A14-F7-P1 | **Moderator elections**: Elections and recall through the kit's elections (KIT-A12-F6-P1), one vote per person. | integration | small | KIT-A12-F6-P1, svc-voting |  |
| MUS-A14-F7-P2 | **Moderator workbench**: Queues for reports, held matches and section flags on the kit's moderator tools (KIT-A10-F6-P3), with both recordings side by side for a match. | integration | medium | KIT-A10-F6-P3, MUS-A14-F1-P1 |  |
| MUS-A14-F7-P3 | **Moderator training and wellbeing**: Training on music rights and the rules (KIT-A25-F2-P1), and care for people who review harmful material (KIT-A10-F6-P4). | integration | small | KIT-A25-F2-P1, KIT-A10-F6-P4 |  |
| MUS-A14-F8-P1 | **Spam uploads**: Limits for new accounts (KIT-A10-F11-P1), and checks for mass uploads of near-identical tracks, silence and noise made only to farm plays. | integration | medium | KIT-A10-F11-P1, MUS-A11-F1-P2 |  |
| MUS-A14-F8-P2 | **Noise and spam rules**: Published rules on tracks of silence or noise, very short tracks and mass uploads, so each removal names its rule like any other. | legal | small | MUS-A14-F2-P1 |  |
| MUS-A14-F9-P1 | **Request desk**: The kit's published process for police and court requests (KIT-A10-F9-P3), with counts in the transparency report. | integration | small | KIT-A10-F9-P3, MUS-A14-F4-P2 |  |
| MUS-A14-F10-P1 | **Second review**: A random sample of decisions gets a second review, with agreement rates published, using the kit's decision sampling (KIT-A10-F20-P1). | integration | small | KIT-A10-F20-P1, MUS-A14-F7-P2 |  |
| MUS-A14-F10-P2 | **Trusted flaggers**: A priority queue for recognised safety and rights bodies, on the kit's trusted flaggers (KIT-A10-F17-P1), which must move up from later for this. | integration | small | KIT-A10-F17-P1, MUS-A14-F1-P1 |  |

## MUS-A15 Personal controls, young people and wellbeing

What each listener decides: hidden artists and tracks, explicit filters and content labels, a private session, notifications, each country's age rules and teen defaults, family settings, and care for your hearing.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A15-F1** Hide what you do not want | Hide an artist or a track, skip explicit lyrics, and filter by content labels. | core | P1 Hide and block; P2 Explicit filter; P3 Content labels your way |
| **MUS-A15-F2** Private listening | Listen privately so nothing shows on your profile or shapes your suggestions. | core | P1 Private session |
| **MUS-A15-F3** Age rules for each country | Know accounts follow the age rules where you live. | core | P1 Age rules; P2 Teen defaults |
| **MUS-A15-F4** Choose what reaches you | Choose which notifications and emails you get. | core | P1 Notification settings |
| **MUS-A15-F5** Family settings | Link family accounts, set limits and approve children's purchases. | next | P1 Family accounts |
| **MUS-A15-F6** Care for your hearing | Get a reminder to rest your ears after long, loud listening. | next | P1 Hearing reminder |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A15-F1-P1 | **Hide and block**: Hide artists and tracks from every list and radio, and block accounts (KIT-A10-F4-P1). | service | small | KIT-A10-F4-P1, MUS-A5-F2-P1 |  |
| MUS-A15-F1-P2 | **Explicit filter**: Skips tracks marked explicit, playing a clean version where the artist uploaded one. | service | small | MUS-A6-F2-P3 |  |
| MUS-A15-F1-P3 | **Content labels your way**: Show, warn or hide by content label, on the kit's labels and filters (KIT-A10-F13-P1, KIT-A10-F13-P2). | integration | small | KIT-A10-F13-P1, KIT-A10-F13-P2 |  |
| MUS-A15-F2-P1 | **Private session**: A switch that keeps plays off the profile and out of suggestions; they still pay artists. | service | small | MUS-A16-F5-P1, MUS-A3-F5-P1 |  |
| MUS-A15-F3-P1 | **Age rules**: Minimum ages and teen defaults per country from the country packs (KIT-A19-F6-P1), using the kit's age checks (KIT-A2-F8-P1) without keeping identity documents. | integration | small | KIT-A2-F8-P1, KIT-A19-F6-P1 |  |
| MUS-A15-F3-P2 | **Teen defaults**: For teens: explicit filter on, no messages from strangers, a private profile and spending limits. | service | small | MUS-A15-F3-P1, MUS-A15-F1-P2 |  |
| MUS-A15-F4-P1 | **Notification settings**: Spare Music's notices on the kit's notification settings and digests (KIT-A8-F5-P1, KIT-A8-F5-P3). | integration | small | KIT-A8-F5-P1, KIT-A8-F5-P3 |  |
| MUS-A15-F5-P1 | **Family accounts**: The kit's family accounts and purchase approvals (KIT-A1-F16-P1, KIT-A1-F16-P2). | integration | small | KIT-A1-F16-P1, KIT-A1-F16-P2 |  |
| MUS-A15-F6-P1 | **Hearing reminder**: An optional reminder after long loud listening on headphones, using the sound levels the device reports, following the World Health Organization's safe listening guidance (to be checked). | library | small | MUS-A2-F1-P1 |  |

## MUS-A16 Audio pipeline, storage and delivery

Turning an upload into music that plays well anywhere: one catalogue store, checking files, measuring loudness, encoding for streaming and download, storing masters safely, delivering at cost, recording each play once, and deleting for good.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A16-F1** One catalogue | Know every artist, release, track and credit is kept in one consistent catalogue. | core | P1 Catalogue service |
| **MUS-A16-F2** Uploads checked and prepared | Know every upload is checked, measured and ready to play within minutes. | core | P1 Ingest pipeline; P2 Loudness measurement; P3 Waveforms |
| **MUS-A16-F3** Plays smoothly anywhere | Hear music start fast and keep playing on slow or patchy connections. | core | P1 Encoding; P2 Streaming delivery; P3 Web player |
| **MUS-A16-F4** Stored and delivered at cost | Know storage and delivery cost as little as possible, shown on the ledger. | core | P1 Master storage; P2 Delivery network; P3 Music cost lines; P4 Cost of music at scale |
| **MUS-A16-F5** Each play recorded once | Know each play and sale is recorded once, the same way, for payouts, charts and stats. | core | P1 Play events; P2 When a play counts; P3 Sales records |
| **MUS-A16-F6** Deleted means deleted | Know music you take down leaves storage and caches on a published timetable. | core | P1 Deletion |
| **MUS-A16-F7** Details from open music data | Have releases matched with open music data, so codes and credits stay right. | next | P1 Open metadata matching |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A16-F1-P1 | **Catalogue service**: Artists, releases, tracks, credits and rights as operations on the kit's operation layer and storage (kit-ops, kit-store), read through the query API. | service | large | kit-ops, kit-store, MUS-A6-F2-P1, MUS-A1-F4-P1 |  |
| MUS-A16-F2-P1 | **Ingest pipeline**: Decodes the upload (KIT-A27-F8-P2), checks it, finds long silence, and starts loudness measurement, waveforms and fingerprinting, as a job that survives restarts (KIT-A15-F14-P1). | service | large | KIT-A27-F8-P2, MUS-A6-F1-P1, KIT-A15-F14-P1 |  |
| MUS-A16-F2-P2 | **Loudness measurement**: Loudness and true peak per track and per album by the published ITU-R BS.1770 method and EBU R 128, stored for the player. | library | small | MUS-A16-F2-P1 |  |
| MUS-A16-F2-P3 | **Waveforms**: A small waveform for each track for the player, with positions read out for screen readers. | library | small | MUS-A16-F2-P1 |  |
| MUS-A16-F3-P1 | **Encoding**: Encodes each track to Opus (royalty-free, BSD-licensed library) at several bitrates for streaming and keeps FLAC for lossless play and downloads, plus any formats the codec study clears (KIT-A27-F8-P1). | library | medium | KIT-A27-F8-P1, KIT-A27-F8-P2, MUS-A16-F2-P1 |  |
| MUS-A16-F3-P2 | **Streaming delivery**: Plays start from the first chunk and step down in quality when the connection drops, over plain HTTP with signed links that expire. | service | medium | MUS-A16-F3-P1, MUS-A16-F4-P2 |  |
| MUS-A16-F3-P3 | **Web player**: Plays in any modern browser with no app, for shared links and embeds. | interface | medium | MUS-A16-F3-P2 |  |
| MUS-A16-F4-P1 | **Master storage**: Lossless masters kept in two places on the kit's storage at cost (KIT-A15-F7-P2), checked against their checksums on a schedule (KIT-A22-F3-P1). | integration | medium | KIT-A15-F7-P2, KIT-A22-F3-P1 |  |
| MUS-A16-F4-P2 | **Delivery network**: Delivery through caches near listeners, from the kit's hosting choice (KIT-A22-F4-P2), compared on price per gigabyte. | integration | medium | KIT-A22-F4-P2 |  |
| MUS-A16-F4-P3 | **Music cost lines**: Monthly cost lines for gigabytes stored and delivered, encoding hours, licences and moderation hours, metered per app (KIT-A13-F2-P2). | integration | small | svc-ledger, KIT-A13-F2-P2 | [K06](../kit/STATUS.md) |
| MUS-A16-F4-P4 | **Cost of music at scale**: Storage and bandwidth per listener and per artist, compared across co-op hosting, caching partners and peer-assisted delivery. | research | medium |  |  |
| MUS-A16-F5-P1 | **Play events**: A play counts after a published number of seconds and is recorded once, with only what payouts and fake-play checks need, then reduced to counts after a published time (KIT-A18-F6-P1). | service | medium | MUS-A16-F3-P2, kit-ops, KIT-A18-F6-P1 |  |
| MUS-A16-F5-P2 | **When a play counts**: The published rule for when a play counts, such as after 30 seconds, and how repeats by one listener count, voted on by artists. | legal | small | MUS-A16-F5-P1, MUS-A21-F1-P1 |  |
| MUS-A16-F5-P3 | **Sales records**: Every sale, pass share and tip as one record that earnings, statements and society reports all read. | data | small | MUS-A9-F2-P1, kit-ops |  |
| MUS-A16-F6-P1 | **Deletion**: Taken-down releases leave caches at once and storage after a short grace period, except copies the law or buyers' downloads need (KIT-A18-F6-P1). | service | small | MUS-A16-F4-P1, MUS-A16-F4-P2, KIT-A18-F6-P1 |  |
| MUS-A16-F7-P1 | **Open metadata matching**: Matches releases with MusicBrainz's core data (CC0) to suggest codes and credits the artist confirms, within its published rate limits and commercial terms; its supplementary data (CC BY-NC-SA) is not used. | integration | medium | MUS-A6-F2-P1, MUS-A6-F2-P2 |  |

## MUS-A17 Open network and moving

Spare Music connects outward: anyone can take their bought music and their data with them, artists can take their whole catalogue and fan list, other open servers can follow artists here, existing music players can connect through an open API, and groups can run their own server.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A17-F1** Take your music with you | Download everything you bought, any time, in open formats with no lock. | core | P1 Bought music export; P2 Artist export |
| **MUS-A17-F2** Learn from open music projects | Know Spare Music is built on what open projects learned, with every licence checked. | core | P1 Study of open music projects; P2 Licence register |
| **MUS-A17-F3** Follow across open networks | Follow Spare Music artists from other open networks, and artists there from here. | next | P1 Federation; P2 Rights over the network |
| **MUS-A17-F4** Use other music players | Use existing music player apps that speak the open OpenSubsonic API. | next | P1 Open player API |
| **MUS-A17-F5** Run your own Spare Music server | Run a server for a scene, a label or a community, connected to the rest. | later | P1 Self-hosting |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A17-F1-P1 | **Bought music export**: All bought music in FLAC or other cleared formats with full tags and artwork, and playlists as M3U and XSPF, through the kit's data export (KIT-A18-F1-P1). | integration | small | KIT-A18-F1-P1, MUS-A9-F2-P2, MUS-A3-F2-P1 |  |
| MUS-A17-F1-P2 | **Artist export**: Artists download their masters, details, credits, splits, statements and fan list in open formats, to move anywhere. | service | small | MUS-A16-F4-P1, MUS-A7-F3-P1, MUS-A12-F3-P1, KIT-A18-F1-P1 |  |
| MUS-A17-F2-P1 | **Study of open music projects**: How Funkwhale, Navidrome, Ampache, MusicBrainz, Mirlo and Resonate are built and where they struggled, reading GPL and AGPL code only to learn, never to copy. | research | large |  |  |
| MUS-A17-F2-P2 | **Licence register**: Spare Music's entries in the one licence register: every open-source library and data set used, its licence and where it came from. | legal | small |  | [A14](../spare-construct/STATUS.md) |
| MUS-A17-F3-P1 | **Federation**: Releases and posts published over ActivityPub, the W3C standard Funkwhale uses for music, so other servers can follow artists; designed from the specification, never from Funkwhale's AGPL code (KIT-A1-F9-P1). | integration | large | MUS-A6-F2-P1, MUS-A12-F2-P1, KIT-A1-F9-P1 |  |
| MUS-A17-F3-P2 | **Rights over the network**: Only release details, previews and links travel to other servers; full audio stays on servers that hold the licences, and AI-use answers travel with each track. | legal | medium | MUS-A17-F3-P1, MUS-A8-F4-P1, MUS-A5-F1-P1 |  |
| MUS-A17-F4-P1 | **Open player API**: An API compatible with the OpenSubsonic specification (Apache-2.0 in its repository; the site's notice to be checked), written from the specification, so existing players can stream bought and saved music. | integration | medium | MUS-A16-F1-P1, MUS-A16-F3-P2 |  |
| MUS-A17-F5-P1 | **Self-hosting**: Spare Music in the kit's self-host package (KIT-A22-F7-P1), with the licence and tax duties that come with selling music on it (KIT-A22-F7-P4). | integration | large | KIT-A22-F7-P1, KIT-A22-F7-P4, MUS-A17-F3-P1 |  |

## MUS-A18 Your data and privacy

Listening history is private by default, nothing is sold and there are no adverts, members can see, download and delete their data, data stays in the right place, and music is kept out of AI training unless the artist says yes.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A18-F1** Download all your data | Download your account, history, playlists and purchases any time. | core | P1 Data export |
| **MUS-A18-F2** No tracking and no adverts | Use Spare Music with no adverts and no data sold, now and later. | core | P1 No-advert rule; P2 Counting without tracking |
| **MUS-A18-F3** Your rights under the law | See, correct and delete your data under your country's privacy law. | core | P1 Privacy requests |
| **MUS-A18-F4** Your music out of AI training | Know your music is never used to train AI unless you choose to allow it. | core | P1 AI training choice; P2 Limits on bulk copying |
| **MUS-A18-F5** Data kept in the right place | Know where your data is stored and who else handles it. | core | P1 Processors and location |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A18-F1-P1 | **Data export**: Spare Music's data in the kit's data export (KIT-A18-F1-P1). | integration | small | KIT-A18-F1-P1 |  |
| MUS-A18-F2-P1 | **No-advert rule**: A protected rule (MUS-A21-F1-P3) against adverts and data sales; an artist's own news to their followers is not an advert. | legal | small | MUS-A21-F1-P3 |  |
| MUS-A18-F2-P2 | **Counting without tracking**: Usage counted with the kit's private usage counts (KIT-A22-F8-P1), never per person beyond what payouts need. | integration | small | KIT-A22-F8-P1 |  |
| MUS-A18-F3-P1 | **Privacy requests**: The kit's privacy page and request clock (KIT-A18-F3-P1, KIT-A18-F3-P2), with overseas privacy duties met (KIT-A18-F12-P1). | integration | small | KIT-A18-F3-P1, KIT-A18-F3-P2, KIT-A18-F12-P1 |  |
| MUS-A18-F4-P1 | **AI training choice**: Artists' music and listeners' history are kept out of AI training by default, with the kit's AI training choice (KIT-A17-F11-P1); the co-op never sells music for training. | integration | small | KIT-A17-F11-P1 |  |
| MUS-A18-F4-P2 | **Limits on bulk copying**: Published limits against bulk downloading of the catalogue, such as for AI training (KIT-A16-F1-P4). | integration | small | KIT-A16-F1-P4, MUS-A16-F3-P2 |  |
| MUS-A18-F5-P1 | **Processors and location**: The kit's processor list and overseas disclosure checks (KIT-A18-F15-P1, KIT-A18-F15-P2). | integration | small | KIT-A18-F15-P1, KIT-A18-F15-P2 |  |

## MUS-A19 Apps, devices and offline

Spare Music in a browser, on phones and computers, on slow connections, with settings that follow you, a first-run guide, and watches and speakers later.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A19-F1** Use it in a browser | Listen, buy and run an artist page in any modern browser. | core | P1 Web app |
| **MUS-A19-F2** Phone apps | Install Spare Music on a phone, with background play and offline music. | core | P1 Phone apps; P2 Background play rules |
| **MUS-A19-F3** Works on slow connections | Keep listening on a slow or patchy connection. | core | P1 Light mode |
| **MUS-A19-F4** Settings that follow you | Have your settings and choices the same on every device. | core | P1 Settings |
| **MUS-A19-F5** Get started | Learn the Human and AI choice and how artists are paid on the first day. | core | P1 First run |
| **MUS-A19-F6** Computer app | Install Spare Music on a computer, with media keys and big libraries. | next | P1 Desktop app |
| **MUS-A19-F7** Watches and smart speakers | Play on a watch or a smart speaker. | later | P1 Watches and speakers |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A19-F1-P1 | **Web app**: The whole app in the browser on the kit's shared components and app frame (KIT-A20-F1-P1, KIT-A20-F1-P2). | integration | medium | KIT-A20-F1-P1, KIT-A20-F1-P2 |  |
| MUS-A19-F2-P1 | **Phone apps**: Phone apps by the kit's chosen approach (KIT-A20-F2-P1, KIT-A20-F2-P2), with background audio, lock screen controls and downloads. | integration | large | KIT-A20-F2-P1, KIT-A20-F2-P2, MUS-A2-F1-P1 | [K04](../kit/STATUS.md) |
| MUS-A19-F2-P2 | **Background play rules**: How each phone platform allows background audio and offline storage for each kind of app, and what that means for the approach. | research | small |  | [K04](../kit/STATUS.md) |
| MUS-A19-F3-P1 | **Light mode**: Lower quality, smaller artwork and cached lists on slow connections (KIT-A19-F5-P1). | integration | small | KIT-A19-F5-P1, MUS-A2-F4-P1 |  |
| MUS-A19-F4-P1 | **Settings**: Settings on the kit's settings framework (KIT-A20-F5-P1, KIT-A20-F5-P2). | integration | small | KIT-A20-F5-P1, KIT-A20-F5-P2 |  |
| MUS-A19-F5-P1 | **First run**: A short first-run guide (KIT-A20-F6-P1) on the sections, how money reaches artists and how to support them. | integration | small | KIT-A20-F6-P1, MUS-A5-F2-P2 |  |
| MUS-A19-F6-P1 | **Desktop app**: A desktop app on the kit's desktop shell and installers (KIT-A20-F2-P4, KIT-A20-F2-P5). | integration | medium | KIT-A20-F2-P4, KIT-A20-F2-P5 |  |
| MUS-A19-F7-P1 | **Watches and speakers**: Watch apps and speaker apps where each maker's terms allow, with offline play on watches. | integration | medium | MUS-A2-F1-P1 |  |

## MUS-A20 Accessibility and languages

Every screen works with a screen reader and a keyboard, lyrics are there as text, the app speaks your language, text and contrast can change, and dates and money follow your country.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A20-F1** Works with screen readers and keyboards | Use every screen, the player included, with a screen reader or a keyboard. | core | P1 Accessible components |
| **MUS-A20-F2** Lyrics as text | Read the words of songs if you are deaf or hard of hearing. | core | P1 Lyrics for access |
| **MUS-A20-F3** Your language | Use Spare Music in your language, with names in their own scripts. | core | P1 Languages |
| **MUS-A20-F4** Comfortable to use | Make text larger, raise contrast and reduce motion. | core | P1 Display settings |
| **MUS-A20-F5** Local formats | See dates, numbers and money written the way your country writes them. | core | P1 Local formats |
| **MUS-A20-F6** Lyrics in your language | Read a translation of the lyrics, marked as a translation. | next | P1 Lyric translations |
| **MUS-A20-F7** Sign language versions | Watch a sign language version of a song where the artist made one. | later | P1 Sign language videos |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A20-F1-P1 | **Accessible components**: The kit's accessible components and testing (KIT-A19-F2-P1, KIT-A19-F2-P2), with the player's position, queue and waveform read out. | integration | small | KIT-A19-F2-P1, KIT-A19-F2-P2, MUS-A2-F1-P1 |  |
| MUS-A20-F2-P1 | **Lyrics for access**: Lyrics as accessible text in step with the music, and transcripts for spoken word with the kit's captions (KIT-A19-F4-P1). | integration | small | MUS-A2-F5-P1, KIT-A19-F4-P1 |  |
| MUS-A20-F3-P1 | **Languages**: Interface languages on the kit's translation framework (KIT-A19-F1-P1); artist names and titles kept in their own script with a sort name; right-to-left layout (KIT-A19-F7-P1). | integration | small | KIT-A19-F1-P1, KIT-A19-F7-P1 |  |
| MUS-A20-F4-P1 | **Display settings**: The kit's display settings (KIT-A19-F3-P1). | integration | small | KIT-A19-F3-P1 |  |
| MUS-A20-F5-P1 | **Local formats**: The kit's local formats (KIT-A19-F1-P2). | integration | small | KIT-A19-F1-P2 |  |
| MUS-A20-F6-P1 | **Lyric translations**: Translations from the artist, or machine translations (KIT-A19-F10-P2) marked as such and only where the songwriter allows them. | integration | small | MUS-A2-F5-P1, KIT-A19-F10-P2 |  |
| MUS-A20-F7-P1 | **Sign language videos**: Artists add a sign language video to a track, shown beside the player. | data | small | MUS-A6-F2-P1 |  |

## MUS-A21 Running it: fees, votes, costs and support

Artists vote on Spare Music's fees and rules, major changes go to a vote of members, the running costs are published every month and set the fee, help is easy to find, the servers are run safely, and performance is measured.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A21-F1** Artists set the fees and rules | Propose and vote on Spare Music's fees and rules, as an artist. | core | P1 Proposals and votes; P2 Who votes on what; P3 Protected promises |
| **MUS-A21-F2** See what it costs | See each month what storage, delivery, licences and moderation cost, and how the fee was set. | core | P1 Music on the cost ledger; P2 Setting the fee |
| **MUS-A21-F3** Get help | Find answers, ask for help, report a bug and see if something is down. | core | P1 Help and support |
| **MUS-A21-F4** Run the servers safely | Admins run, watch, back up and secure the servers, with every action logged. | core | P1 Admin, monitoring and security |
| **MUS-A21-F5** Legal set-up | Read plain terms of use and know which legal body runs the service. | core | P1 Terms of use; P2 Insurance |
| **MUS-A21-F6** Measured performance | Know how fast music starts and pages load, measured and published. | core | P1 Performance budgets |
| **MUS-A21-F7** Fair storage for big catalogues | Upload freely, with very large catalogues paying for their storage at cost if artists vote for it. | next | P1 Storage allowance |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A21-F1-P1 | **Proposals and votes**: Spare Music proposals and results through the kit's ballots (KIT-A12-F3-P1), applied as operations (KIT-A12-F4-P1), one vote per person. | integration | small | svc-voting, KIT-A12-F3-P1, KIT-A12-F4-P1 | [K01](../kit/STATUS.md) |
| MUS-A21-F1-P2 | **Who votes on what**: Artists vote on Spare Music's fees and rules, listeners have a say on listener matters such as the pass, and major changes go to a vote of members, set in the kit's electorates (KIT-A12-F5-P2). | legal | medium | KIT-A12-F5-P2 | [K01](../kit/STATUS.md) |
| MUS-A21-F1-P3 | **Protected promises**: Artists keeping everything except running costs, no adverts, no paid placement and removing only what the law requires change only by a vote of members (KIT-A12-F5-P1). | legal | small | KIT-A12-F5-P1 | [K01](../kit/STATUS.md) |
| MUS-A21-F2-P1 | **Music on the cost ledger**: Spare Music's monthly costs and fee on the kit's ledger pages and fee explainer (KIT-A13-F1-P3, KIT-A13-F3-P2). | integration | small | KIT-A13-F1-P3, KIT-A13-F3-P2, MUS-A16-F4-P3 | [K06](../kit/STATUS.md) |
| MUS-A21-F2-P2 | **Setting the fee**: The fee per sale and per stream set from last period's costs on the kit's fee setting and prices by use (KIT-A13-F3-P1, KIT-A13-F3-P6), with any surplus lowering the next period's fee. | integration | small | KIT-A13-F3-P1, KIT-A13-F3-P6 | [K06](../kit/STATUS.md) |
| MUS-A21-F3-P1 | **Help and support**: The kit's help centre, support cases and status page (KIT-A21-F1-P1, KIT-A21-F2-P1, KIT-A21-F5-P1), with music rights explained in plain words, and the kit's Report a bug button. | integration | small | KIT-A21-F1-P1, KIT-A21-F2-P1, KIT-A21-F5-P1 |  |
| MUS-A21-F4-P1 | **Admin, monitoring and security**: The kit's admin console, monitoring and security reviews (KIT-A22-F6-P1, KIT-A22-F1-P1, KIT-A22-F5-P3). | integration | small | KIT-A22-F6-P1, KIT-A22-F1-P1, KIT-A22-F5-P3 |  |
| MUS-A21-F5-P1 | **Terms of use**: Plain terms for listeners, and the artist agreement, matching the rules and the co-op's legal entity (KIT-A18-F4-P2). | legal | medium | KIT-A18-F4-P2, MUS-A7-F1-P3 | [K01](../kit/STATUS.md) |
| MUS-A21-F5-P2 | **Insurance**: Insurance for copyright and defamation claims over what artists upload, on the kit's co-op insurance (KIT-A14-F17-P1). | integration | small | KIT-A14-F17-P1 |  |
| MUS-A21-F6-P1 | **Performance budgets**: Targets for time to first sound, skip time and page load, measured on real connections in several countries (KIT-A22-F2-P5). | integration | small | KIT-A22-F2-P5 |  |
| MUS-A21-F7-P1 | **Storage allowance**: A free storage allowance with at-cost storage beyond it, if artists vote for it, on the kit's per-account metering (KIT-A13-F2-P3). | integration | small | KIT-A13-F2-P3, KIT-A15-F7-P2 |  |

## MUS-A22 Headless API, plugins and AI agents

Everything a person can do, a script or AI agent can do through the same commands, with agent labels, guards and approvals, actions only people may take, plugins, a developer API, webhooks and a register of where Spare Music uses AI.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MUS-A22-F1** Everything by command | Do anything in Spare Music through the same commands the app uses. | core | P1 Command set; P2 Query API |
| **MUS-A22-F2** Use your own AI | Let your own AI assistant build playlists, manage releases or answer fans, within permissions you set. | core | P1 MCP server; P2 Guards and approvals; P3 People-only actions |
| **MUS-A22-F3** Plugins | Add visualisers, equalisers, listening loggers and new ways to browse, each showing what it can access. | next | P1 Plugin points |
| **MUS-A22-F4** Developer API | Build apps on Spare Music with scoped permissions and good docs. | next | P1 Public API |
| **MUS-A22-F5** See where Spare Music uses AI | See every place Spare Music uses AI and ask a person to review a decision. | next | P1 AI use register |
| **MUS-A22-F6** Webhooks | Get a call to your own server when you make a sale or gain a supporter. | later | P1 Webhooks |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MUS-A22-F1-P1 | **Command set**: Every action (play, buy, upload, declare, split, report, moderate) as a command on the kit's operation layer and command API (KIT-A16-F1-P1). | library | large | kit-ops, KIT-A16-F1-P1, MUS-A16-F1-P1 |  |
| MUS-A22-F1-P2 | **Query API**: Read the catalogue, library, playlists, stats and statements through one query API that respects every permission. | library | large | MUS-A16-F1-P1, MUS-A22-F1-P1 |  |
| MUS-A22-F2-P1 | **MCP server**: Spare Music's commands on the kit's MCP server and authorisation (KIT-A17-F1-P1, KIT-A17-F1-P3). | integration | medium | kit-mcp, KIT-A17-F1-P1, KIT-A17-F1-P3, MUS-A22-F1-P1 |  |
| MUS-A22-F2-P2 | **Guards and approvals**: Titles, lyrics and posts reach an agent as data, never as instructions (KIT-A17-F5-P1); a person approves purchases, payout and split changes, and releases (KIT-A17-F2-P1). | integration | small | KIT-A17-F5-P1, KIT-A17-F2-P1, MUS-A22-F2-P1 |  |
| MUS-A22-F2-P3 | **People-only actions**: Votes, appeals, rights declarations and split approvals are made only by a person (KIT-A17-F9-P2). | integration | small | KIT-A17-F9-P2 |  |
| MUS-A22-F3-P1 | **Plugin points**: Plugin hooks for the player, library views and artist tools on the kit's plugin host (KIT-A16-F5-P2). | library | large | kit-plugin, KIT-A16-F5-P2 |  |
| MUS-A22-F4-P1 | **Public API**: Sign-in through the kit's provider, scoped tokens, rate limits and versions (KIT-A1-F6-P1, KIT-A16-F1-P2, KIT-A16-F1-P3, KIT-A16-F9-P1). | integration | medium | KIT-A1-F6-P1, KIT-A16-F1-P2, KIT-A16-F1-P3, KIT-A16-F9-P1, MUS-A22-F1-P2 |  |
| MUS-A22-F5-P1 | **AI use register**: Detection hints, fake-play signals, translations and suggestions in the kit's AI use register (KIT-A17-F10-P1), with review by a person (KIT-A17-F10-P2). | integration | small | KIT-A17-F10-P1, KIT-A17-F10-P2 |  |
| MUS-A22-F6-P1 | **Webhooks**: Spare Music events on the kit's signed webhooks (KIT-A16-F2-P1). | integration | small | KIT-A16-F2-P1 |  |

## Questions for assessing this map

- Would the core features alone make a credible switch for someone leaving Spotify, Apple Music, SoundCloud or Bandcamp, given that the big labels' catalogues will not be there at first?
- Should the first release sell like Bandcamp (buy to keep, pay what you like, free streams) and add the listening pass later, or launch with the pass (MUS-A9-F5)?
- If there is a pass, should each listener's money go only to the artists they played, or be pooled by share of all plays, and how does each choice change the reward for fake plays (MUS-A9-F5-P3, MUS-A11-F5)?
- Does a society member's own upload still need that society's licence in each launch country, what would those licences cost an at-cost service, and is the rule for who carries the cost fair (MUS-A8-F1-P2, MUS-A8-F3-P2)?
- Can Spare Music report plays and sales to each country's societies in their formats without a large staff, and which countries should come first?
- Are covers allowed at launch only where a licence covers them (MUS-A7-F4), and is asking for proof for every sample workable for independent artists?
- Which fingerprinting method is accurate enough, licensed for the core (Chromaprint is LGPL-2.1 as a whole) and clear of patents, and how many held matches can people review each week (MUS-A11-F1)?
- Which copy protection, if any: a closed DRM lock, a watermark in each copy, or none, given that nothing stops someone recording the sound as it plays (MUS-A11-F6, MUS-A11-F7)?
- Does 'remove only what the law requires' hold up for music, where hate lyrics, impersonation and AI copies of real voices sit close to the line (MUS-A14-F3-P4, MUS-A5-F5)?
- Can the Human and AI sections be enforced fairly when AI is used for mastering, drums or backing vocals, and where should AI-assisted end (MUS-A5-F1-P3)?
- Can artists keep everything except running costs on iPhone and Android, given the app stores' payment rules (MUS-A9-F7)?
- How is tax handled when downloads and passes are sold across borders, and which kit parts must move up to core for that, such as tax rules per country (KIT-A4-F7) and currencies per country (KIT-A3-F14)?
- Who votes on what: artists on fees and rules, listeners on the pass, and members on major changes (MUS-A21-F1-P2)?
- What do lossless masters and delivery cost per artist and per listener each month, and who covers free listening (MUS-A16-F4-P4, MUS-A9-F6)?
- Which candidate kit parts should move down from Spare Music so other apps can use them: audio fingerprinting, rights packs, society reporting and split payouts?
- Which parts carry legal risk under 'Built legally, or not at all': fingerprint and watermark patents, the DDEX licence terms, the OpenSubsonic site notice, MusicBrainz's non-commercial supplementary data and AcoustID's non-commercial service?
- Is this map the right size for contributors with spare tokens, and what is the smallest slice worth building and demonstrating first?
- What numbers would prove the demo works, such as time to first sound on a slow phone connection, time from upload to live, and running cost per listener per month?
