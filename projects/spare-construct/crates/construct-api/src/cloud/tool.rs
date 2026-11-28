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

/// The one list of the command layer's tools, in order: `module::Type` for each, where the type
/// implements [`Tool`]. Expands to `Tools`, whose functions are the dispatch the list implies.
macro_rules! tools {
    ($($module:ident :: $tool:ident),+ $(,)?) => {
        /// Every tool of the command layer, in the order of the list in `cloud/mod.rs`
        /// ([`tool`](crate::cloud::tool)).
        pub struct Tools;

        impl Tools {
            /// Every tool, in the list's order.
            pub const LISTED: &'static [$crate::cloud::tool::Listed] = &[$(
                $crate::cloud::tool::Listed {
                    name: <$module::$tool as $crate::cloud::tool::Tool>::NAME,
                    commands: <$module::$tool as $crate::cloud::tool::Tool>::COMMANDS,
                    steps: <$module::$tool as $crate::cloud::tool::Tool>::STEPS,
                    guards: <$module::$tool as $crate::cloud::tool::Tool>::GUARDS,
                    undo_order: <$module::$tool as $crate::cloud::tool::Tool>::UNDO_ORDER,
                },
            )+];

            /// Whether a tool runs this command.
            #[must_use]
            pub fn is_command(cmd: &str) -> bool {
                Self::LISTED.iter().any(|tool| tool.commands.contains(&cmd))
            }

            /// Whether an applied entry of this command is a step a person can undo.
            #[must_use]
            pub fn is_step(cmd: &str) -> bool {
                Self::LISTED.iter().any(|tool| tool.steps.contains(&cmd))
            }

            /// Runs the command with the tool whose command it is, behind the guards of the
            /// tools that guard that tool; `None` when it is no tool's.
            ///
            /// # Errors
            ///
            /// A store error; a command that cannot be made is the outcome's error instead.
            pub async fn apply<T: ::kit_store::Txn>(
                txn: &mut T,
                project: &::kit_core::Id,
                context: &::kit_sync::Context<'_>,
                edit: &::kit_ops::Edit,
                claim_ms: u64,
            ) -> Result<Option<::kit_sync::Outcome>, ::kit_store::StoreError> {
                let cmd = edit.cmd.as_str();
                $(
                    if <$module::$tool as $crate::cloud::tool::Tool>::COMMANDS.contains(&cmd) {
                        return Self::run::<$module::$tool, T>(txn, project, context, edit, claim_ms)
                            .await
                            .map(Some);
                    }
                )+
                Ok(None)
            }

            /// Runs the command with `U`, behind the guards of the tools that guard it.
            ///
            /// # Errors
            ///
            /// A store error; a command that cannot be made is the outcome's error instead.
            pub async fn run<U: $crate::cloud::tool::Tool, T: ::kit_store::Txn>(
                txn: &mut T,
                project: &::kit_core::Id,
                context: &::kit_sync::Context<'_>,
                edit: &::kit_ops::Edit,
                claim_ms: u64,
            ) -> Result<::kit_sync::Outcome, ::kit_store::StoreError> {
                if let Err(refused) = Self::guard(U::NAME, txn, project, edit).await? {
                    return Ok(Err(refused));
                }
                U::apply(txn, project, context, edit, claim_ms).await
            }

            /// Asks the guard of every tool that guards the tool named `of`, in the list's order;
            /// the first refusal is the answer.
            ///
            /// # Errors
            ///
            /// A store error.
            pub async fn guard<T: ::kit_store::Txn>(
                of: &str,
                txn: &mut T,
                project: &::kit_core::Id,
                edit: &::kit_ops::Edit,
            ) -> Result<Result<(), String>, ::kit_store::StoreError> {
                $(
                    if <$module::$tool as $crate::cloud::tool::Tool>::GUARDS.contains(&of) {
                        if let Err(refused) =
                            <$module::$tool as $crate::cloud::tool::Tool>::guard(txn, project, edit)
                                .await?
                        {
                            return Ok(Err(refused));
                        }
                    }
                )+
                Ok(Ok(()))
            }

            /// Asks every tool's undo rules, in the order of their [`UNDO_ORDER`] (the list's
            /// where two are equal); the first refusal is the answer.
            ///
            /// [`UNDO_ORDER`]: crate::cloud::tool::Tool::UNDO_ORDER
            ///
            /// # Errors
            ///
            /// A store error.
            pub async fn undo_rules<T: ::kit_store::Txn>(
                txn: &mut T,
                project: &::kit_core::Id,
                document: &::kit_core::Id,
                written: &[::construct_model::element::Element],
                deleted: &[::kit_core::Id],
                whole: bool,
            ) -> Result<Result<(), String>, ::kit_store::StoreError> {
                let mut order: Vec<&$crate::cloud::tool::Listed> = Self::LISTED.iter().collect();
                // A stable sort: two equal places keep the list's order.
                order.sort_by_key(|tool| tool.undo_order);
                for listed in order {
                    $(
                        if listed.name == <$module::$tool as $crate::cloud::tool::Tool>::NAME {
                            if let Err(why) =
                                <$module::$tool as $crate::cloud::tool::Tool>::undo_rules(
                                    txn, project, document, written, deleted, whole,
                                )
                                .await?
                            {
                                return Ok(Err(why));
                            }
                        }
                    )+
                }
                Ok(Ok(()))
            }
        }
    };
}

pub(crate) use tools;

#[cfg(test)]
mod tests {
    use crate::cloud::Tools;
    use std::collections::BTreeSet;

    /// The names of the tools that guard the tool named `of`, in the order their guards are asked.
    fn guarding(of: &str) -> Vec<&'static str> {
        Tools::LISTED
            .iter()
            .filter(|tool| tool.guards.contains(&of))
            .map(|tool| tool.name)
            .collect()
    }

    /// `names` with only those of `known` left, in their order.
    fn only<'a>(names: &[&'a str], known: &[&str]) -> Vec<&'a str> {
        names
            .iter()
            .copied()
            .filter(|name| known.contains(name))
            .collect()
    }

    #[test]
    fn every_name_is_kebab_case_and_one_tools() {
        let mut seen = BTreeSet::new();
        for tool in Tools::LISTED {
            assert!(
                !tool.name.is_empty()
                    && !tool.name.starts_with('-')
                    && !tool.name.ends_with('-')
                    && tool
                        .name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '-'),
                "{} is not kebab-case",
                tool.name
            );
            assert!(seen.insert(tool.name), "two tools are named {}", tool.name);
        }
    }

    /// So the list's order never decides which tool runs a command.
    #[test]
    fn no_command_is_two_tools() {
        let mut seen = BTreeSet::new();
        for tool in Tools::LISTED {
            assert!(!tool.commands.is_empty(), "{} runs no command", tool.name);
            for cmd in tool.commands {
                assert!(seen.insert(*cmd), "{cmd} is two tools' command");
            }
        }
    }

    #[test]
    fn every_step_is_its_tools_command() {
        for tool in Tools::LISTED {
            for step in tool.steps {
                assert!(
                    tool.commands.contains(step),
                    "{step} is a step of {} but not its command",
                    tool.name
                );
            }
        }
    }

    #[test]
    fn every_guarded_tool_is_listed() {
        for tool in Tools::LISTED {
            for of in tool.guards {
                assert!(
                    Tools::LISTED.iter().any(|other| other.name == *of),
                    "{} guards {of}, which is not listed",
                    tool.name
                );
            }
        }
    }

    /// The chains the list replaced (piece A97) were written by hand in these orders; the tools
    /// that were in them are still asked in them, so no refusal's words changed. A tool added
    /// since may come anywhere.
    #[test]
    fn the_chains_keep_the_order_they_were_written_in() {
        let element = [
            "view",
            "schedule",
            "dimension",
            "sheet",
            "family",
            "opening",
            "hosting",
            "room",
            "floor",
            "wall",
            "roof",
            "stair",
            "curtain-wall",
        ];
        assert_eq!(only(&guarding("element"), &element), element);
        let parameter = ["floor", "wall", "roof", "curtain-wall"];
        assert_eq!(only(&guarding("parameter"), &parameter), parameter);
        let hosting = ["family", "opening", "curtain-wall"];
        assert_eq!(only(&guarding("hosting"), &hosting), hosting);

        let mut undo: Vec<_> = Tools::LISTED
            .iter()
            .filter(|tool| tool.undo_order != u16::MAX)
            .collect();
        undo.sort_by_key(|tool| tool.undo_order);
        let undo: Vec<&str> = undo.iter().map(|tool| tool.name).collect();
        let written = [
            "hosting",
            "family-loading",
            "level",
            "view",
            "schedule",
            "sheet",
            "dimension",
        ];
        assert_eq!(only(&undo, &written), written);
    }

    /// The generic commands and the steps, as they were before the list.
    #[test]
    fn the_commands_and_steps_are_the_tools() {
        for cmd in [
            "element.create",
            "element.claim",
            "element.release",
            "parameter.bind",
            "type.create",
            "element.param",
            "edit.undo",
            "edit.redo",
            "hosted.place",
            "family.load",
            "family.place",
        ] {
            assert!(Tools::is_command(cmd), "{cmd}");
        }
        for cmd in [
            "document.checkpoint",
            "model.add",
            "link.add",
            "element.other",
        ] {
            assert!(!Tools::is_command(cmd), "{cmd}");
        }
        for cmd in ["element.claim", "element.release", "document.checkpoint"] {
            assert!(!Tools::is_step(cmd), "{cmd}");
        }
        for cmd in ["element.move", "edit.undo", "family.reload", "type.set"] {
            assert!(Tools::is_step(cmd), "{cmd}");
        }
    }
}
