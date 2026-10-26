use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const WORKSPACE_EDITOR: Uuid = Uuid::from_u128(0xde8b_500a_12d8_4bc0_8381_8bbf_dc9f_e34f);
pub const FILES_EDITOR: Uuid = Uuid::from_u128(0x9026_bab5_b23e_4b81_a693_d3a2_5090_5f9e);
pub const LINUX_DESKTOP_EDITOR: Uuid = Uuid::from_u128(0x378d_6c68_d52a_4781_addb_fde9_a8af_f2e2);
pub const WORKSPACE_TYPE: Uuid = Uuid::from_u128(0x776f_726b_7370_6163_652d_626c_6f63_6b01);
pub const VIEW_TYPE: Uuid = Uuid::from_u128(0x6564_6974_6f72_2d76_6965_772d_626c_6b31);
pub const RECENTS: &str = "recents";
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
