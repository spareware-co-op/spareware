# Shared service: the store

The real backend for [`kit-store`](../../crates/kit-store): TiKV for pieces, and anything `object_store` speaks for files (RustFS, S3, Google Cloud Storage, a folder). Note [K08](../../notes/K08-cloud-foundation.md) says why it sits apart from the kit's crates.

`tests/promises.rs` holds TiKV to what `kit-store` promises of a store, and `tests/load.rs` puts the direct `tikv-client` route under load. Both need a running TiKV: `scripts/run-checklist.sh` in [Spare Construct's server](../../../spare-construct/server) runs them.
