//! `Probe`, a block type whose merge over parts reaches past what it was handed: it reads the
//! part its content names (any part, of any block) and makes a call other than `read-part`
//! under the request, and fails with what the host answered both, so a test sees the host
//! refuse them.

use be_block::{
    BlockContent, ContentError, LiveEdit, Loads, Merge, MergeParts, PartReader, PartRef,
};
use be_block_commit::MergeResult;
use be_sdk_type_api::guest::host;
use be_sdk_type_api::ops::guest as ops;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Probe {
    /// The part its merge reads.
    pub target: Option<PartRef>,
}

impl BlockContent for Probe {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x9e0b_e000_0000_4000_8000_7e57_0000_0011);

    const LOADS: Loads = Loads::FirstPart;

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("probe"))
    }
}

impl LiveEdit for Probe {
    type Op = Option<PartRef>;

    fn apply(&mut self, operation: &Option<PartRef>) {
        self.target = *operation;
    }
}

impl Merge for Probe {
    fn merge3(_: &Self, ours: &Self, _: &Self) -> MergeResult<Self> {
        MergeResult::Clean(ours.clone())
    }
}

/// What a probe's merge fails with: what the host answered its call other than `read-part`
/// and its read of the target, as `call: …; read: …`.
impl MergeParts for Probe {
    fn merge_parts(
        _: Self,
        ours: Self,
        _: Self,
        parts: PartReader,
    ) -> impl Future<Output = Result<MergeResult<Self>, String>> + 'static {
        let request = ops::request();
        async move {
            let put = || host::put_part(b"a probe".to_vec());
            let called = match request {
                Some(request) => ops::with_parent(request, put),
                None => put(),
            }
            .await;
            let read = match ours.target {
                Some(target) => parts.read_all(target).await.map(|bytes| bytes.len()),
                None => Err("no target".to_owned()),
            };
            Err(format!("call: {called:?}; read: {read:?}"))
        }
    }
}

be_sdk_type_guest::be_block_types![Probe { merge_parts }];
