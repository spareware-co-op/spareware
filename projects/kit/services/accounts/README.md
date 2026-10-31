# Shared service: accounts

One Spareware login for every app, plus who belongs where and what they may do.

- **The directory:** people, hubs, projects, teams and who is in each, with bridge permissions between hubs. It changes only through its own edit log.
- **Access:** rows say who belongs where, and [Cedar rules](policy/access.cedar) decide on every request.
- **Sign-in:** OpenID Connect, with sessions kept in the store so a restart signs nobody out.

Runs once for every app; apps connect to it rather than carrying their own copy. Today it is a library the [test stack](../../test-stack) links in. Identity and licence checks come later. Map: [ARCHITECTURE.md](../../ARCHITECTURE.md), "Accounts"; decisions in note [K08](../../notes/K08-cloud-foundation.md).
