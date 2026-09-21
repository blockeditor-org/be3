use be_commit::MergeResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BlockContent, ContentError, LiveEdit, Merge};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CounterContent {
    count: i64,
}

impl CounterContent {
    pub const fn new(count: i64) -> Self {
        Self { count }
    }

    pub const fn count(&self) -> i64 {
        self.count
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CounterOp {
    Add { by: i64 },
    Reset,
}

impl BlockContent for CounterContent {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x636f_756e_7465_722d_626c_6f63_6b2d_7402);

    fn encode(&self) -> Vec<u8> {
        self.count.to_le_bytes().to_vec()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let bytes: [u8; 8] = bytes
            .try_into()
            .map_err(|_| ContentError::Malformed("a counter is eight bytes"))?;
        Ok(Self {
            count: i64::from_le_bytes(bytes),
        })
    }
}

impl LiveEdit for CounterContent {
    type Op = CounterOp;

    fn apply(&mut self, operation: &Self::Op) {
        self.count = match operation {
            CounterOp::Add { by } => self.count.saturating_add(*by),
            CounterOp::Reset => 0,
        };
    }
}

impl Merge for CounterContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        let ours_delta = ours.count.saturating_sub(base.count);
        let theirs_delta = theirs.count.saturating_sub(base.count);
        MergeResult::Clean(Self {
            count: base
                .count
                .saturating_add(ours_delta)
                .saturating_add(theirs_delta),
        })
    }
}
