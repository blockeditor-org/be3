use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
pub enum BlockRef {
    Direct(Uuid),
    RepoRelative { repo: Uuid, eternal_id: Uuid },
}

impl BlockRef {
    pub fn as_direct(&self) -> Option<Uuid> {
        match self {
            Self::Direct(id) => Some(*id),
            Self::RepoRelative { .. } => None,
        }
    }
}
