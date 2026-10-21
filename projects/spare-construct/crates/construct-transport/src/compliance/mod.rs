//! Compliance checks against transport rules and standards (port of features/compliance.js and features/compliance-extra.js).
//! Spare Construct: transport model.
//!
//! The first country pack: New Zealand's rules and guidance as versioned data and the checks that read a project against
//! them. The Compliance module covers the regulatory pathway and regime, who reviews and who is affected, the Auckland Unitary
//! Plan E27 trigger screening, the integrated transport assessment (ITA) scope, access and frontage, state highways, the
//! Future Development Strategy, further-information and hearing clocks, expert sign-off, the standing disclaimers and the
//! 87-item regulatory checklist, with the report sections, exports and CSV stamping built on them. [`extra`] (Compliance+)
//! covers the scope and wording linter, precinct staging, mitigation and conditions, options and alternatives, the Fast-track
//! package, plan change, speed limits, ANOP, parking, subdivision, policy alignment, time-of-use screening and the hand-over packs.
//!
//! The checks read a project, its results and the user's entries the way the prototype does, as loose JSON ([`jv::V`], objects
//! keep their key order), and give their answers in the same shape, so they can be compared with the prototype field by field.
//! The rule tables are the prototype's own, embedded from `data/` ([`data`]); the regular expressions the rules use run on
//! [`rx`]. Not ported: the Compliance tab's forms and event handling, the inspector sections, the map layer and the live
//! lookups of Auckland Council and NZTA map services (they need the network and the page).

pub mod data;
pub mod dates;
pub mod jv;
pub mod net;
pub mod rx;
pub mod record;

pub use record::{
    active_sc, base_sc, blank, fill, in_auckland, num, numf, periods_of, pw, recheck, record_outputs, regime, res_of, settings, stamp, state, state_mut,
    used_docs,
};
pub use dates::today;
pub mod engine;
pub mod docs;
pub mod html;
pub mod authority;
pub mod clocks;
pub mod extra;
pub mod geom;

pub use authority::{authorities, link_authority, link_authority_with, parse_precincts, site_info, site_point, sh_of};
pub use clocks::{add_months, add_working_days, clock_for, clocks, holidays, is_working_day, rfi_template, working_days_between};
pub mod e27;
pub mod fds;

pub use e27::e27;
pub use fds::fds_check;
pub mod expert;
pub mod rfi;

pub use rfi::lint;
