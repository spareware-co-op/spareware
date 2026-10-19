# Spare Movies: architecture map

*Stage: Idea. Named, not mapped yet.*

Replaces Netflix. Filmmakers sell their films and shows the way they want, and viewers come to one place to watch them. Like every Spareware app, the filmmakers keep all of the money except the running costs.

## Goals

- Filmmakers choose how their work is sold.
- One place for people to find and watch it.
- Filmmakers keep everything except running costs, shown on the open cost ledger.
- Films are hard to copy and pass around without paying.

## Human and AI sections

The catalogue has two sections: **Human**, for films and shows people made themselves, and **AI**, for ones made with AI. Each viewer chooses to see one, the other, or both, and can change it at any time.

- **Declared by the filmmaker:** every film and show says whether AI was used. Accounts that are AI agents are labelled and can publish only in the AI section.
- **Checked, not assumed:** uploads are checked for content credentials (C2PA, an open standard for recording how media was made) and for AI-detection signals. Signals are hints for moderators, never automatic verdicts.
- **Fixable in the open:** members can flag a film in the wrong section; moderators move it with the reason in the public log, and the filmmaker can appeal.
- **Fair to filmmakers:** using AI isn't a penalty; it just goes in its own section, so human work isn't buried.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| How viewers pay | Each filmmaker sets their own way: buy to keep, rent, pay what you like, or free; or one subscription for everything; or both | To be decided |
| How a subscription is shared out | Each week, what comes in is shared out by views, so the most watched that week get the biggest share; or each viewer's fee goes only to the filmmakers that viewer watched | To be decided |
| Stopping copies | Locked playback (DRM): Google's Widevine, Apple's FairPlay and Microsoft's PlayReady are closed systems and each needs an agreement with that company; or a hidden watermark in each copy, so a leaked copy leads back to the account it came from; or no lock. Nothing stops someone recording the screen as it plays, so no option is complete | To be decided |
