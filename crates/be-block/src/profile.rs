use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const WORKSPACE_EDITOR: Uuid = Uuid::from_u128(0x776f_726b_7370_6163_652d_7569_2d30_3031);
pub const FILES_EDITOR: Uuid = Uuid::from_u128(0x6669_6c65_2d74_7265_652d_626c_6f63_6b01);
pub const VIEW_EDITORS: [Uuid; 2] = [WORKSPACE_EDITOR, FILES_EDITOR];
pub const RECENTS: &str = "recents";
pub const SESSION: &str = "session";
pub const MAX_RECENT: usize = 20;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecentBlock {
    pub block: Uuid,
    pub block_type: Uuid,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Recents(pub Vec<RecentBlock>);

impl Recents {
    pub fn visit(&self, block: Uuid, block_type: Uuid) -> Option<Self> {
        let entry = RecentBlock { block, block_type };
        if self.0.first() == Some(&entry) {
            return None;
        }
        let recent = std::iter::once(entry)
            .chain(self.0.iter().copied().filter(|held| held.block != block))
            .take(MAX_RECENT)
            .collect();
        Some(Self(recent))
    }

    pub fn forget(&self, block: Uuid) -> Option<Self> {
        self.0.iter().any(|held| held.block == block).then(|| {
            Self(
                self.0
                    .iter()
                    .copied()
                    .filter(|held| held.block != block)
                    .collect(),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Session {
    pub desktop: bool,
}
