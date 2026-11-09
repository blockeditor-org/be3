//! Block types for tests of the type protocol, linked only by tests (a dev-dependency):
//! `sections`, which loads and merges by parts, and `probe`, which reaches past what a
//! stateless request may.

pub mod probe;
pub mod sections;
