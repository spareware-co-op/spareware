//! One directory log, two sets of commands: the kit's people and Spare Construct's hubs, projects
//! and teams. `construct-api` can't see the accounts service, whose dependencies are too heavy for
//! a browser's build, so this is where they meet, and where Cedar answers for both.

use construct_api::cloud::directory as spaces;
use construct_api::cloud::{Access, Target};
use kit_accounts::{Cedar, people};
use kit_ops::Edit;
use kit_store::{Kv, Scope, StoreError};
use kit_sync::{Commands, Context, Outcome};
use std::sync::Arc;

/// The directory's command version, recorded in each entry.
const VERSION: u32 = 1;

pub struct DirectoryCommands<K: Kv> {
    access: Arc<Access<K, Cedar>>,
}

impl<K: Kv> DirectoryCommands<K> {
    #[must_use]
    pub fn new(access: Arc<Access<K, Cedar>>) -> Self {
        Self { access }
    }
}

impl<K: Kv> Commands<K> for DirectoryCommands<K> {
    fn version(&self) -> u32 {
        VERSION
    }

    async fn permit(&self, _scope: &Scope, edit: &Edit) -> Result<Result<(), String>, StoreError> {
        let allowed = match edit.cmd.as_str() {
            // Allowed once, on an empty log: `people::apply` checks that.
            "directory.start" => true,
            "login.bind" => {
                let target = Target::Person(&edit.person);
                self.access
                    .allowed(&edit.person, "login.bind", target, None)
                    .await?
            }
            _ => {
                self.access
                    .allowed(&edit.person, "directory.edit", Target::Directory, None)
                    .await?
            }
        };
        Ok(if allowed {
            Ok(())
        } else {
            Err("You can't change the directory.".to_owned())
        })
    }

    fn permit_group(&self, edit: &Edit) -> String {
        format!("{}/{}", edit.person, edit.cmd)
    }

    async fn apply(
        &self,
        txn: &mut K::Txn,
        context: &Context<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        if people::handles(&edit.cmd) {
            people::apply(txn, context, edit).await
        } else {
            spaces::apply(txn, edit).await
        }
    }
}
