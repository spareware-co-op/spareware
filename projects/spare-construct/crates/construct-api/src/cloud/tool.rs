//! The tool contract of the command layer (CON-A8, note A96 "The command layer"; piece A97).
//!
//! A tool of a project's commands is one module of [`crate::cloud`] that implements [`Tool`]: its
//! name, the commands it runs, the generic commands its guard is asked before, and its undo rule.
//! Each tool is named once, in the ordered `tools!` list in `cloud/mod.rs`, which expands to
//! [`Tools`](crate::cloud::Tools): whether a command is a tool's, whether it is a step of undo,
//! running it behind the guards of the tools that guard it, and every tool's undo rule. So the
//! orderer's commands ([`commands`](crate::cloud::commands)) and undo
//! ([`undo`](crate::cloud::undo)) name no tool.
//!
//! **Adding a tool** adds its module (`cloud/<tool>.rs`, implementing [`Tool`]) and two lines in
//! `cloud/mod.rs`: its `pub mod` line, which Rust needs (it is not made by the macro, because
//! rustfmt does not reach a module declared inside a macro), and its line at the end of the
//! `tools!` list. Nothing else outside the tool changes.
//!
//! **Order.** The list's order is the order the guards are asked in, and the order a command is
//! looked for (no command is two tools', which a test checks, so that order never decides which
//! tool runs it). A refusal's words depend on which guard answers first, so the order is part of
//! the answer, and it is the list's, the same on every run. The undo rules were written in
//! another order than the guards, so a tool gives its undo rule's place itself
//! ([`Tool::UNDO_ORDER`]); two equal places keep the list's order.
//!
//! **Why a list and not registration at link time** (note A96): the commands are generic over the
//! store's transaction, and a generic function cannot be registered as a value without boxing
//! every call behind a type-erased transaction; and a linker's order is no order the golden rules
//! allow in anything compared. A declarative macro of our own is the whole mechanism: it writes
//! out, from the one list, the chains that were written by hand.
//!
//! Depends on kit-store's transaction, kit-ops' edit and kit-sync's outcome only; nothing here
//! reads a clock.

use construct_model::element::Element;
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{StoreError, Txn};
use kit_sync::{Context, Outcome};

/// A tool of the command layer: its commands, its guard of the generic commands, its undo rule.
///
/// Every hook but [`Tool::apply`] has a default that says nothing, so a tool writes only what it
/// has. A hook is called directly, as the function it calls was, so a store error and a refusal go
/// where they went.
pub trait Tool {
    /// Its name: kebab-case, unique in the list. Other tools name it in their [`Tool::GUARDS`].
    const NAME: &'static str;
    /// The commands it runs, every one behind `model.edit`, in the same access group as an
    /// element's.
    const COMMANDS: &'static [&'static str];
    /// Those of its commands whose applied entries are steps a person can undo
    /// ([`crate::cloud::undo`]): all of them, unless it says otherwise.
    const STEPS: &'static [&'static str] = Self::COMMANDS;
    /// The tools, by [`Tool::NAME`], whose commands its [`Tool::guard`] is asked before: the
    /// generic commands (an element's, a parameter's or a type's, hosting's) that could reach its
    /// own records.
    const GUARDS: &'static [&'static str] = &[];
    /// Where its [`Tool::undo_rules`] are asked among the tools': lowest first, and the list's
    /// order where two are equal.
    const UNDO_ORDER: u16 = u16::MAX;

    /// Runs one of its commands.
    ///
    /// # Errors
    ///
    /// A store error; a command that cannot be made is the outcome's error instead.
    fn apply<T: Txn>(
        txn: &mut T,
        project: &Id,
        context: &Context<'_>,
        edit: &Edit,
        claim_ms: u64,
    ) -> impl Future<Output = Result<Outcome, StoreError>> + Send;

    /// Refuses a command of a tool it guards ([`Tool::GUARDS`]) that would go behind its own
    /// commands, naming the command to use. The inner error is the refusal.
    ///
    /// # Errors
    ///
    /// A store error.
    fn guard<T: Txn>(
        txn: &mut T,
        project: &Id,
        edit: &Edit,
    ) -> impl Future<Output = Result<Result<(), String>, StoreError>> + Send {
        let _ = (txn, project, edit);
        async { Ok(Ok(())) }
    }

    /// Turns back an undo or a redo that would leave its records inconsistent, asked once it has
    /// written `written` and deleted the pieces `deleted` in `document`; `whole` when it wrote a
    /// type or the parameter table. The inner error is the refusal, without the undo's own
    /// opening words.
    ///
    /// # Errors
    ///
    /// A store error.
    fn undo_rules<T: Txn>(
        txn: &mut T,
        project: &Id,
        document: &Id,
        written: &[Element],
        deleted: &[Id],
        whole: bool,
    ) -> impl Future<Output = Result<Result<(), String>, StoreError>> + Send {
        let _ = (txn, project, document, written, deleted, whole);
        async { Ok(Ok(())) }
    }
}

/// What the list says of one tool, as data: for the checks on the list, and for what reads the
/// tools without running them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Listed {
    /// [`Tool::NAME`].
    pub name: &'static str,
    /// [`Tool::COMMANDS`].
    pub commands: &'static [&'static str],
    /// [`Tool::STEPS`].
    pub steps: &'static [&'static str],
    /// [`Tool::GUARDS`].
    pub guards: &'static [&'static str],
    /// [`Tool::UNDO_ORDER`].
    pub undo_order: u16,
}
