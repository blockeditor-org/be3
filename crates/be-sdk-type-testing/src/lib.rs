//! Block types for tests of the type protocol, linked only by tests (a dev-dependency):
//! `sections`, which loads and merges by parts, `probe`, which reaches past what a
//! stateless request may, and `tally`, whose session state grows until the order restarts.

pub mod probe;
pub mod sections;
pub mod tally;
