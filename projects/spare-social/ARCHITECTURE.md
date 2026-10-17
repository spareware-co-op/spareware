# Spare Social: architecture map

*Working name. Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

One app for short videos, long videos and channels, photos and stories, posts, groups, events and messages, replacing TikTok, YouTube, Instagram and Facebook together. Members own it, it runs at cost with no adverts, your feed is yours, and nothing is hidden by a secret algorithm. It speaks the open social protocols, so members can follow and be followed across other open networks from day one.

## What "not censored" means here

Spare Social removes only what the law requires to be removed, by published rules, in the open. Everything else stays up, and each person decides what they see.

| Spare Social does | Spare Social doesn't |
| --- | --- |
| Shows your feed in time order, or by a ranking you choose whose rules are published | Hide or bury posts with a secret algorithm, or shadow-ban anyone |
| Lets you filter what you see: mute, block, content labels, community filters you subscribe to | Decide for you what you're allowed to see beyond the law |
| Removes illegal content only (child abuse material, terrorist and objectionable material under NZ law, credible threats, copyright takedowns) | Remove lawful posts because they're unpopular, political or inconvenient |
| Has moderators elected by members, a right of appeal, and a public log of every removal and the rule it broke | Moderate in secret |
| Runs at cost: no adverts, no selling data | Make money from attention |

## The parts

| Part | Replaces | What it does |
| --- | --- | --- |
| Short videos | TikTok, Reels, Shorts | Vertical video feed, sounds, duets and remixes |
| Videos and channels | YouTube | Long video, channels, playlists, subscriptions, chapters |
| Photos and stories | Instagram | Photo posts, carousels, stories that expire |
| Posts and groups | Facebook | Text posts, comments, groups, pages, events |
| Messages | Messenger, DMs | One-to-one and group chat |
| Live | YouTube Live, TikTok Live | Live streaming (later) |
| Creator support | Platform revenue shares | Members tip or subscribe to creators directly; creators keep everything except payment fees |

## What it uses from the shared kit

Accounts (one Spareware login), payments (creator support), messaging, trust (reports, labels, appeals), member voting (moderator elections and rules), the open cost ledger, sync, plugins and the MCP server. Video uses the shared `engine-media`, the same video engine as Spare Studio.

## Low level

- **Open protocols:** ActivityPub (the W3C standard used by Mastodon, PeerTube and Pixelfed) and the AT Protocol (used by Bluesky). Members can move their account and followers elsewhere. Which to support first is open (piece M01).
- **Video pipeline:** upload, then transcode into several sizes for streaming, with thumbnails and captions. Codec choice matters legally: AV1 is royalty-free; H.264 and HEVC carry patent licensing (piece M03).
- **Delivery:** video storage and bandwidth are by far the biggest running cost. Options include peer-assisted delivery (viewers share pieces of a video, as PeerTube does) and member-hosted servers (piece M06).
- **Feeds:** time order by default; custom feeds anyone can publish and anyone can choose, with their rules visible (piece M05).
- **Safety tooling:** matching uploads against known illegal material, age checks, reporting and appeal flows, built as kit services so every app benefits (piece M07).
- **Learn from:** PeerTube, Pixelfed and Mastodon are AGPL, so we read how they work and write our own code; we never copy it. Bluesky's AT Protocol libraries need their licences checked before any use (piece M08).

## The hard parts

- **Cost of video:** storage and bandwidth grow with every upload; the at-cost model has to cover them openly.
- **Legal duties:** NZ law (including the Harmful Digital Communications Act 2015 and the Films, Videos, and Publications Classification Act 1993) and overseas laws for overseas members. Needs a lawyer's view (piece M02).
- **Getting people on:** a network is only useful once people's friends are there; open protocols help by connecting to existing networks.
- **Spam, bots and minors:** need strong tools from day one.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Name | Spare Social; another name | Spare Social |
| Protocol | ActivityPub; AT Protocol; both; our own | Research first (piece M01) |
| First part | Short video; photos; posts | Research first |
| Video delivery | Co-op hosted at cost; peer-assisted; member servers | Research first (piece M06) |
| Live streaming | First release; later | Later |

The work board is [STATUS.md](STATUS.md).
