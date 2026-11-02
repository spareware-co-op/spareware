//! A project's commands: what the orderer runs for this program, and Cedar's say on each. A
//! command that isn't listed in `permit` is refused, so a new command can't arrive unguarded.

use crate::cloud::access::{Access, Target};
use crate::cloud::elements;
use crate::cloud::spaces::{self, AddLink, AddModel, Bridge, Publish, Take, folders};
use construct_model::geometry::Edition;
use construct_model::keys::Place;
use construct_model::rows::{LinkKind, Model, Project};
use kit_core::Id;
use kit_ops::{Decider, Edit};
use kit_store::{Kv, Scope, StoreError, read_row};
use kit_sync::{Commands, Context, Outcome};
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// The version of these commands, recorded in each log entry.
const VERSION: u32 = 1;
const NO_ACCESS: &str = "You don't have access to this.";

pub struct ProjectCommands<K: Kv, D: Decider> {
    kv: K,
    access: Arc<Access<K, D>>,
    claim_ms: u64,
    /// The edition this server builds with, which a new document starts on.
    edition: Edition,
}

impl<K: Kv, D: Decider> ProjectCommands<K, D> {
    pub fn new(kv: K, access: Arc<Access<K, D>>, claim_ms: u64, edition: Edition) -> Self {
        Self {
            kv,
            access,
            claim_ms,
            edition,
        }
    }

    async fn allowed(
        &self,
        person: &Id,
        action: &str,
        target: Target<'_>,
    ) -> Result<bool, StoreError> {
        self.access.allowed(person, action, target, None).await
    }

    async fn may_bridge(
        &self,
        project: &Id,
        edit: &Edit,
        args: &Bridge,
    ) -> Result<bool, StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        let target: Option<Project> = read_row(
            &mut snapshot,
            &Place::Project(args.to_project.clone()).key(),
        )
        .await?;
        let Some(target) = target else {
            return Ok(false);
        };
        let shared = folders::shared();
        let package = Target::Package(project, &args.team, &shared, &edit.target);
        self.access
            .allowed(&edit.person, "package.bridge", package, Some(&target.hub))
            .await
    }

    async fn may_link(
        &self,
        project: &Id,
        edit: &Edit,
        args: &AddLink,
    ) -> Result<bool, StoreError> {
        let model = Target::Model(project, &edit.target);
        if !self.allowed(&edit.person, "model.link", model).await? {
            return Ok(false);
        }
        match args.kind {
            LinkKind::Live => {
                let source = Target::Model(project, &args.source);
                self.allowed(&edit.person, "model.read", source).await
            }
            LinkKind::Shared => {
                let Some(team) = &args.team else {
                    return Ok(false);
                };
                let shared = folders::shared();
                let source = Target::Package(project, team, &shared, &args.source);
                self.allowed(&edit.person, "package.read", source).await
            }
            LinkKind::Consumed => {
                // `apply` looks only in the model's own team's folder.
                let mut snapshot = self.kv.snapshot().await?;
                let place = Place::Model {
                    project: project.clone(),
                    model: edit.target.clone(),
                };
                let host: Option<Model> = read_row(&mut snapshot, &place.key()).await?;
                let Some(host) = host else { return Ok(false) };
                let consumed = folders::consumed();
                let source = Target::Package(project, &host.team, &consumed, &args.source);
                self.allowed(&edit.person, "package.read", source).await
            }
        }
    }
}

fn read<T: DeserializeOwned>(edit: &Edit) -> Result<T, String> {
    serde_json::from_value(edit.args.clone()).map_err(|e| format!("That edit isn't valid: {e}"))
}

impl<K: Kv, D: Decider> Commands<K> for ProjectCommands<K, D> {
    fn version(&self) -> u32 {
        VERSION
    }

    async fn permit(&self, scope: &Scope, edit: &Edit) -> Result<Result<(), String>, StoreError> {
        let Scope::Project(project) = scope else {
            return Ok(Err("These commands act on a project.".to_owned()));
        };
        let person = &edit.person;
        let allowed = match edit.cmd.as_str() {
            "element.create" | "element.move" | "element.set" | "element.delete"
            | "element.claim" | "element.release" => {
                self.allowed(person, "model.edit", Target::Model(project, &edit.target))
                    .await?
            }
            "document.checkpoint" => {
                let model = Target::Model(project, &edit.target);
                self.allowed(person, "document.checkpoint", model).await?
            }
            "model.add" => match read::<AddModel>(edit) {
                Ok(args) => {
                    self.allowed(person, "model.add", Target::Team(project, &args.team))
                        .await?
                }
                Err(reason) => return Ok(Err(reason)),
            },
            "package.publish" => match read::<Publish>(edit) {
                Ok(args) => {
                    let team = Target::Team(project, &args.team);
                    let model = Target::Model(project, &edit.target);
                    self.allowed(person, "package.publish", team).await?
                        && self.allowed(person, "model.read", model).await?
                }
                Err(reason) => return Ok(Err(reason)),
            },
            "package.bridge" => match read::<Bridge>(edit) {
                Ok(args) => self.may_bridge(project, edit, &args).await?,
                Err(reason) => return Ok(Err(reason)),
            },
            "package.take" => match read::<Take>(edit) {
                Ok(args) => {
                    self.allowed(person, "package.consume", Target::Team(project, &args.team))
                        .await?
                }
                Err(reason) => return Ok(Err(reason)),
            },
            "link.add" => match read::<AddLink>(edit) {
                Ok(args) => self.may_link(project, edit, &args).await?,
                Err(reason) => return Ok(Err(reason)),
            },
            other => return Ok(Err(format!("Unknown command {other}."))),
        };
        Ok(if allowed {
            Ok(())
        } else {
            Err(NO_ACCESS.to_owned())
        })
    }

    /// Every element edit of a model has the same answer, so a batch asks Cedar once.
    fn permit_group(&self, edit: &Edit) -> String {
        if edit.cmd.starts_with("element.") {
            format!("{}/element/{}", edit.person, edit.target)
        } else {
            format!("{}/{}/{}/{}", edit.person, edit.cmd, edit.target, edit.args)
        }
    }

    async fn apply(
        &self,
        txn: &mut K::Txn,
        context: &Context<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        let Scope::Project(project) = context.scope else {
            return Ok(Err("These commands act on a project.".to_owned()));
        };
        if edit.cmd.starts_with("element.") {
            elements::apply(txn, project, context, edit, self.claim_ms).await
        } else if edit.cmd == "document.checkpoint" {
            Ok(elements::checkpoint(edit))
        } else {
            spaces::apply(txn, project, context, edit, &self.edition).await
        }
    }
}
