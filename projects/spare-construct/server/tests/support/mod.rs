//! What the in-process tests share: a whole server on the test store.

use construct_api::cloud::{POLICY, ProjectCommands, SCHEMA};
use construct_model::geometry::Geometry;
use kit_accounts::{Accounts, Cedar, SignInSettings};
use kit_store::memory::{Memory, MemoryFiles};
use kit_sync::{Clock, Orderers, SystemClock};
use spare_server::directory::DirectoryCommands;
use spare_server::service::{Access, Service};
use std::sync::Arc;

pub fn build(
    store: &Memory,
    files: &MemoryFiles,
    geometry: Arc<dyn Geometry>,
) -> Service<Memory, MemoryFiles> {
    let cedar = Cedar::new(POLICY, SCHEMA).unwrap();
    let access = Arc::new(Access::new(store.clone(), cedar));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let commands = Arc::new(DirectoryCommands::new(access.clone()));
    let directory = Orderers::new(store.clone(), commands, clock.clone(), 50);
    let settings = SignInSettings {
        issuer: "http://localhost:9".to_owned(),
        client_id: String::new(),
        client_secret: String::new(),
        public_url: "http://localhost:8".to_owned(),
    };
    let accounts = Accounts::new(store.clone(), clock.clone(), settings, directory);
    let commands = ProjectCommands::new(store.clone(), access.clone(), 30_000, geometry.edition());
    let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
    Service::new(
        store.clone(),
        access,
        accounts,
        orderers,
        clock,
        files.clone(),
        geometry,
    )
}
