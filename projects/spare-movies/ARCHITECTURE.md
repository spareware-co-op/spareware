# Spare Movies: architecture map

*Stage: Idea. Named, not mapped yet.*

Replaces Netflix. Filmmakers sell their films and shows the way they want, and viewers come to one place to watch them. Like every Spareware app, the filmmakers keep all of the money except the running costs.

## Goals

- Filmmakers choose how their work is sold.
- One place for people to find and watch it.
- Filmmakers keep everything except running costs, shown on the open cost ledger.
- Films are hard to copy and pass around without paying.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| How viewers pay | Each filmmaker sets their own way: buy to keep, rent, pay what you like, or free; or one subscription for everything; or both | To be decided |
| How a subscription is shared out | Each week, what comes in is shared out by views, so the most watched that week get the biggest share; or each viewer's fee goes only to the filmmakers that viewer watched | To be decided |
| Stopping copies | Locked playback (DRM): Google's Widevine, Apple's FairPlay and Microsoft's PlayReady are closed systems and each needs an agreement with that company; or a hidden watermark in each copy, so a leaked copy leads back to the account it came from; or no lock. Nothing stops someone recording the screen as it plays, so no option is complete | To be decided |
