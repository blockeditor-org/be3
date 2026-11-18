//! `Ledger`, a block type with a signed log beside its content: each entry counts one in what
//! it shows (`LogFold`), so a test sees what reaches the log and what a description folds.

use be_block_commit::MergeResult;
use be_block_content::{BlockContent, ContentError, LiveEdit, Merge};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Ledger {
    pub count: i64,
}

impl BlockContent for Ledger {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x9e0b_e000_0000_4000_8000_7e57_0000_0031);

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("ledger"))
    }
}

impl LiveEdit for Ledger {
    type Op = i64;

    fn apply(&mut self, by: &i64) {
        self.count += by;
    }
}

impl Merge for Ledger {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        MergeResult::Clean(Self {
            count: ours.count + theirs.count - base.count,
        })
    }
}

/// Each entry of the log counts one, in the log's order.
impl be_block_content::LogFold for Ledger {
    fn fold(&mut self, _: &be_block_content::LogItem) {
        self.count += 1;
    }
}

be_sdk_type_guest::be_block_types![Ledger { log }];
