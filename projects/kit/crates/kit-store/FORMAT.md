# The Spareware cloud store format, version 1

This is a published format under [EXTENDING.md](../../../../EXTENDING.md), section 3. Everything a browser program keeps on the co-op cloud is stored this way. **We may stop writing an old version. We never stop reading it.** This page is the specification; [`src/key.rs`](src/key.rs), [`src/row.rs`](src/row.rs) and `kit-ops`'s [`src/entry.rs`](../kit-ops/src/entry.rs) are the reader library; [`kit-ops/tests/samples`](../kit-ops/tests/samples) holds recorded entries that every later version must still read.

The store is an ordered map from keys to values, with transactions. TiKV holds it today, but nothing here depends on that: a backup is this same map as lines of JSON.

## Versions

Three things carry a version, and each is checked when it is read:

| What | Where the version is | Now |
| --- | --- | --- |
| The key layout | The value at `meta/format`, written once by the directory's first entry | 1 |
| Each row | `{"v":1,"row":...}` around the value | 1 |
| Each log entry | The `v` field | 1 |

A program that finds a version newer than it knows refuses that row rather than guessing. To change any of these, add a new version: teach the reader the new one beside the old, add recorded samples, and never rewrite what is stored.

## IDs

An ID is 1 to 64 characters from `A-Z a-z 0-9 . _ -`, and starts with a letter or digit. So a SHA-256 in hexadecimal is a valid ID, and no ID can hold a `/` or a `:`, so nothing needs escaping and one project's keys can never run into another's.

New IDs are 128 bits written in 26 characters of Crockford's base 32: 48 bits of milliseconds since 1970, then 80 random bits, as ULID lays them out. Sorted as text they sort by time.

## Keys

Keys are plain ASCII, with parts split by `/`. Each key starts with its scope. Numbers appear in one spelling only: no `+`, no leading zeros, no `-0`. Log numbers are always 20 digits, so byte order is log order. A key that doesn't match this table isn't part of the format.

### Each log

`<scope>` is `dir` for the directory or `p:<project>` for a project.

| Key | Value |
| --- | --- |
| `<scope>/head` | The last log number given out. Every commit that adds an entry changes this key, which is what gives the log one order |
| `<scope>/log/<n>` | A log entry, not wrapped as a row |
| `<scope>/client/<person>/<client>` | The last edit taken from one client: `{"counter":n,"seq":n}` |

### The store

| Key | Value |
| --- | --- |
| `meta/format` | The key layout's version |

### A project

| Key | Value |
| --- | --- |
| `p:<project>/record/<hash>` | A piece's data, named by the SHA-256 of these bytes. A version is never rewritten |
| `p:<project>/piece/<document>/<piece>` | The hash of the piece's current record |
| `p:<project>/claim/<document>/<piece>` | Who is working on a piece, until when |

### The directory

| Key | Value |
| --- | --- |
| `dir/person/<person>` | A person. It holds no email address |
| `dir/email/<hash>` | Who an address belongs to, under the SHA-256 of the lowercased address |
| `dir/login/<hash>` | Who a login belongs to, under the SHA-256 of its issuer and subject |
| `acl/admin/<person>` | This person may change the directory. A row's presence is the fact |

### A program's own tables

A program adds tables under the same three roots: `p:<project>/<table>/...`, `dir/<table>/...` and `acl/<table>/...`, with no table name the kit uses. It builds their keys with `kit_store::table` and `Scope::table`, so the parts stay IDs and the rules on this page hold, and it publishes them in a format page of its own. Spare Construct's is [construct-model's FORMAT.md](../../../spare-construct/crates/construct-model/FORMAT.md).

### Sign-in

Not kept forever, and not in any log, because these are secrets that expire.

| Key | Value |
| --- | --- |
| `auth/session/<hash>` | A signed-in browser, under the SHA-256 of its cookie |
| `auth/pending/<hash>` | A sign-in on its way to the provider and back |

## Rows

Every value except a log entry is JSON in the form `{"v":1,"row":<value>}`. JSON, so a row stays readable in decades with no Spareware program to hand. Field names and meanings are in the row types: here `kit-accounts`'s `rows.rs`, and a program's in its own format page.

Numbers in anything hashed are whole: lengths are millimetres. A hash must never depend on how a float prints.

## Log entries

An entry is JSON, one object, with its fields in this order:

| Field | Meaning |
| --- | --- |
| `v` | The entry format: 1 |
| `seq` | Its number in the log, from 1, with no gaps |
| `client`, `counter` | Who sent it and that client's own count, which goes up by one with each edit |
| `op_id` | `<client>.<counter>`: stays the same through a retry |
| `author` | The person |
| `base` | The last log number the client had seen |
| `author_ms` | The client's clock, for history. Nothing runs on it |
| `orderer_ms` | The orderer's clock: the only time a command may read |
| `target` | What it acted on: a document, or whatever else a command says |
| `cmd`, `command_version` | The command, and the version of the commands it ran under |
| `args` | Its arguments, kept for history, undo and AI audit |
| `changes` | `{"piece","before","after"}`: which record a piece moved between. `null` for none |
| `claims` | `{"piece","claim"}`: a claim taken, or `null` when let go |
| `content_id` | SHA-256 of `changes` as written |
| `failed` | `null`, or why the edit was turned back. A turned-back edit stays in the log |

An edit that couldn't be committed is not in the log at all: it stays with its client.

`kit_sync::verify_log` reads a log back and checks all of this: no gaps, every content ID right, each change following from the one before, and replaying the log giving exactly the pieces stored.
