use super::*;

use be_block_client::ClientError;
use be_block_protocol::ErrorCode;

use crate::version::{Due, Failure, Statuses};

const BLOCK: Uuid = Uuid::from_u128(0xb10c);
const INPUTS: crate::version::Signature = (1, 1, 1, 1);
const CHANGED: crate::version::Signature = (2, 1, 1, 1);

#[test]
fn a_version_status_that_failed_waits_for_its_inputs_or_its_kind_of_news() {
    let mut statuses = Statuses::default();
    assert_eq!(statuses.due(BLOCK, INPUTS, false), Due::Compute);

    let shown = statuses.failed(
        BLOCK,
        INPUTS,
        Failure::client(ClientError::Locked(Uuid::from_u128(0x6e7))),
    );
    assert!(
        shown.as_deref().is_some_and(|shown| shown.starts_with("Could not read this block's versions")),
        "a lasting failure shows why at once: {shown:?}"
    );
    assert_eq!(
        statuses.due(BLOCK, INPUTS, false),
        Due::Waiting { transient: false },
        "a wake that changed none of its inputs does not compute it again"
    );
    assert_eq!(
        statuses.due(BLOCK, INPUTS, true),
        Due::Waiting { transient: false },
        "nor does the backoff, which is for transient failures"
    );
    assert_eq!(
        statuses.due(BLOCK, CHANGED, false),
        Due::Compute,
        "new inputs (a key arriving moves them too) compute it again"
    );
    assert_eq!(
        statuses.computed(BLOCK, CHANGED),
        shown,
        "and once it computes, the error it showed is the one to clear"
    );
    assert_eq!(statuses.due(BLOCK, CHANGED, false), Due::Current);

    let missing = || {
        Failure::client(ClientError::Refused(
            ErrorCode::ObjectNotFound,
            "not uploaded yet".to_owned(),
        ))
    };
    statuses.failed(BLOCK, CHANGED, missing());
    assert_eq!(
        statuses.due(BLOCK, CHANGED, false),
        Due::Waiting { transient: false }
    );
    statuses.announced();
    assert_eq!(
        statuses.due(BLOCK, CHANGED, false),
        Due::Compute,
        "an object the server lacked is tried again once a head is announced"
    );
}
