//! Block types for tests of the type protocol, linked only by tests (a dev-dependency):
//! `sections`, which loads and merges by parts, `probe`, which reaches past what a
//! stateless request may, `tally`, whose session state grows until the order restarts, and
//! `ledger`, which keeps a signed log.

pub mod ledger;
pub mod probe;
pub mod sections;
pub mod tally;
