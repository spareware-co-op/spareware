# Shared service: accounts

One Spareware login for every app, plus the people it knows and a way to ask what they may do.

- **People:** who exists, and which addresses and logins are theirs. They change only through the directory's edit log.
- **Access:** [Cedar](src/cedar.rs), answering `kit_ops::Decider`. A program brings its own rules and schema, and asks in Cedar's JSON. This service knows no hub or team: where people belong is each program's. Spare Construct's is in [`construct-api`](../../../spare-construct/crates/construct-api/src/cloud/access.rs).
- **Sign-in:** OpenID Connect, with sessions kept in the store so a restart signs nobody out.

Runs once for every app; apps connect to it rather than carrying their own copy. Today it is a library [Spare Construct's server](../../../spare-construct/server) links in. Identity and licence checks come later. Map: [ARCHITECTURE.md](../../ARCHITECTURE.md), "Accounts"; decisions in note [K08](../../notes/K08-cloud-foundation.md).
