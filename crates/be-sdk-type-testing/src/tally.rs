//! `Tally`, a counter whose session state can be made as large as a test needs, so the order
//! restarts once it is sealed (`RESTART_STATE_BYTES`): a test of what a restart does to edits
//! on their way. It keeps a signed log beside, which each entry counts one in (`LogFold`).

use be_block_commit::MergeResult;
use be_block_content::{BlockContent, ContentError, LiveEdit, Merge};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Tally {
    pub count: i64,
    /// How many bytes its session state holds: only the session keeps them, so a restart,
    /// which loads the content from a commit, drops them.
    #[serde(skip)]
    pub state: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TallyOp {
    Add(i64),
    /// Grows the session state by this many bytes.
    Grow(usize),
}

impl BlockContent for Tally {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x9e0b_e000_0000_4000_8000_7e57_0000_0021);

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("tally"))
    }
}

impl LiveEdit for Tally {
    type Op = TallyOp;

    fn apply(&mut self, operation: &TallyOp) {
        match operation {
            TallyOp::Add(by) => self.count += by,
            TallyOp::Grow(by) => self.state += by,
        }
    }

    fn session_state(&self) -> Vec<u8> {
        vec![0; self.state]
    }

    fn adopt_session_state(&mut self, state: &[u8]) -> Result<(), ContentError> {
        self.state = state.len();
        Ok(())
    }
}

impl Merge for Tally {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        MergeResult::Clean(Self {
            count: ours.count + theirs.count - base.count,
            state: ours.state,
        })
    }
}

/// A tally keeps a signed log too: each entry counts one, in the log's order.
impl be_block_content::LogFold for Tally {
    fn fold(&mut self, _: &be_block_content::LogItem) {
        self.count += 1;
    }
}

be_sdk_type_guest::be_block_types![Tally { log }];
