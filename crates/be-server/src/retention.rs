//! How many old states of a block the server keeps: the organization's policy, or a shorter one
//! built in for a content type whose old states are worth little. The server runs no type
//! modules and has no type registration, so the table is here, beside the content types it
//! knows (`be_block_graph`), and no call or setting fills it.

use be_block::OrganizationPolicies;
use be_block_commit::RetentionPolicy;
use be_block_commit::retention::{Bucket, DAY, HOUR, MINUTE};
use be_block_graph::SEARCH_INDEX_TYPE;
use uuid::Uuid;

/// A type's own policy, where it has one.
fn of_type(content_type: Uuid) -> Option<RetentionPolicy> {
    match content_type {
        // A few recent states: an old index still holds the terms of blocks that left its tree.
        SEARCH_INDEX_TYPE => Some(RetentionPolicy {
            keep_all_within: 0,
            buckets: vec![
                Bucket {
                    until_age: HOUR,
                    interval: 10 * MINUTE,
                },
                Bucket {
                    until_age: DAY,
                    interval: 6 * HOUR,
                },
            ],
            drop_after: Some(DAY),
        }),
        _ => None,
    }
}

/// The policy for a block of `content_type`: its type's, or else the default, and never keeping
/// less than the organization's `history_kept_for`.
pub fn policy(content_type: Uuid, organization: &OrganizationPolicies) -> RetentionPolicy {
    of_type(content_type)
        .unwrap_or_default()
        .keeping_all_within(organization.history_kept_for())
}

#[cfg(test)]
mod tests;
