//! The commands that change who exists. "Every admin action is an operation in the log", says the
//! kit's map, so the directory has an edit log like a project's, and these commands are the only
//! way to write its rows. They are functions, not a whole set of commands, because a program runs
//! its own directory commands in the same log, and answers `permit` for all of them: it holds the
//! rules, so this never decides who may do what.
//!
//! A new store has nobody who could say yes, so the first entry is special: `directory.start`
//! creates the format record and the first administrator, and is turned back once the log isn't
//! empty. There is no other way in, for the seed data or anything else.

use crate::rows::Person;
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{FORMAT, Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Outcome};
use serde::Deserialize;

/// The server's own account: the first administrator, and the author of what the server itself
/// does to the directory, such as applying `STACK_PEOPLE`.
pub const SYSTEM: &str = "system";

/// Whether a command is one of these.
#[must_use]
pub fn handles(cmd: &str) -> bool {
    matches!(
        cmd,
        "directory.start" | "person.add" | "person.email" | "login.bind"
    )
}

#[derive(Deserialize)]
struct AddPerson {
    #[serde(with = "kit_ops::id")]
    id: Id,
    name: String,
    /// The SHA-256 of the lowercased address. The address itself is never stored.
    #[serde(with = "kit_ops::id")]
    email_hash: Id,
}

#[derive(Deserialize)]
struct SetEmail {
    #[serde(with = "kit_ops::id")]
    person: Id,
    #[serde(with = "kit_ops::id")]
    email_hash: Id,
}

#[derive(Deserialize)]
struct BindLogin {
    #[serde(with = "kit_ops::id")]
    login: Id,
    #[serde(with = "kit_ops::id")]
    email_hash: Id,
}

/// Reads a command's arguments, or turns the edit back saying why.
macro_rules! args {
    ($edit:expr, $kind:ty) => {
        match serde_json::from_value::<$kind>($edit.args.clone()) {
            Ok(args) => args,
            Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
        }
    };
}

async fn put<T: Txn>(
    txn: &mut T,
    place: Place,
    row: &impl serde::Serialize,
) -> Result<(), StoreError> {
    txn.put(place.key(), to_row(row)?).await
}

/// Runs one of these commands.
pub async fn apply<T: Txn>(
    txn: &mut T,
    context: &Context<'_>,
    edit: &Edit,
) -> Result<Outcome, StoreError> {
    match edit.cmd.as_str() {
        "directory.start" => {
            if context.head != 0 {
                return Ok(Err("The directory has already been started.".to_owned()));
            }
            put(txn, Place::Format, &FORMAT).await?;
            let admin = Person {
                id: edit.person.clone(),
                name: "System".to_owned(),
            };
            put(txn, Place::Person(admin.id.clone()), &admin).await?;
            put(txn, Place::Admin(admin.id), &true).await?;
        }
        "person.add" => {
            let args = args!(edit, AddPerson);
            let row = Person {
                id: args.id.clone(),
                name: args.name,
            };
            put(txn, Place::Person(args.id.clone()), &row).await?;
            put(txn, Place::Email(args.email_hash), &args.id.as_str()).await?;
        }
        "person.email" => {
            let args = args!(edit, SetEmail);
            let person = Place::Person(args.person.clone());
            if txn.get(&person.key()).await?.is_none() {
                return Ok(Err("There's no such person.".to_owned()));
            }
            put(txn, Place::Email(args.email_hash), &args.person.as_str()).await?;
        }
        "login.bind" => {
            let args = args!(edit, BindLogin);
            // The address must already belong to the person binding it.
            let owner: Option<String> = read_row(txn, &Place::Email(args.email_hash).key()).await?;
            if owner.as_deref() != Some(edit.person.as_str()) {
                return Ok(Err("That address belongs to someone else.".to_owned()));
            }
            put(txn, Place::Login(args.login), &edit.person.as_str()).await?;
        }
        other => return Ok(Err(format!("Unknown command {other}."))),
    }
    Ok(Ok(Applied::default()))
}
