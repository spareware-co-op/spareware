# Spare Music: architecture map

*Stage: Idea. Named, not mapped yet.*

Replaces Spotify. Musicians sell their music the way they want, and listeners come to one place to play it. Like every Spareware app, the musicians keep all of the money except the running costs.

## Goals

- Musicians choose how their music is sold.
- One place for people to find and listen to it.
- Musicians keep everything except running costs, shown on the open cost ledger.
- Music is hard to copy and pass around without paying.

## Human and AI sections

The catalogue has two sections: **Human**, for music people made themselves, and **AI**, for music made with AI. Each listener chooses to see one, the other, or both, and can change it at any time.

- **Declared by the musician:** every track says whether AI was used. Accounts that are AI agents are labelled and can publish only in the AI section.
- **Checked, not assumed:** uploads are checked for content credentials (C2PA, an open standard for recording how media was made) and for AI-detection signals. Signals are hints for moderators, never automatic verdicts.
- **Fixable in the open:** members can flag a track in the wrong section; moderators move it with the reason in the public log, and the musician can appeal.
- **Fair to musicians:** using AI isn't a penalty; it just goes in its own section, so human work isn't buried.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| How listeners pay | Each musician sets their own way: buy to keep, rent, pay what you like, or free; or one subscription for everything; or both | To be decided |
| How a subscription is shared out | Each week, what comes in is shared out by plays, so the most played that week get the biggest share (Spotify pools its money and splits it by share of all plays); or each listener's fee goes only to the musicians that listener played (SoundCloud's "fan-powered royalties") | To be decided |
| Stopping copies | Locked playback (DRM): Google's Widevine, Apple's FairPlay and Microsoft's PlayReady are closed systems and each needs an agreement with that company; or a hidden watermark in each copy, so a leaked copy leads back to the account it came from; or no lock, like Bandcamp's downloads. Nothing stops someone recording the sound as it plays, so no option is complete | To be decided |
