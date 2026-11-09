use be_block_commit::retention::DAY;
use be_block_commit::{CommitId, CommitSummary, plan_retention};

use super::*;

fn commit(seed: u8) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(&[seed]))
}

mod every_type_keeps_everything_inside_the_organizations_history;
