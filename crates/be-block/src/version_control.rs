use be_commit::CommitId;
use be_model::{Document, List, Map, Model};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BlockMetadata, Root};

pub const MAIN_BRANCH: &str = "main";

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Repository {
    pub branches: Map<String, CommitId>,
    pub upstream: Option<Uuid>,
}

impl Repository {
    pub fn branch(&self, name: &str) -> Option<CommitId> {
        self.branches.get(name).copied()
    }

    pub fn with_branch(&self, name: &str, head: Option<CommitId>) -> Self {
        let mut branches: std::collections::BTreeMap<String, CommitId> = (*self.branches).clone();
        match head {
            Some(head) => branches.insert(name.to_owned(), head),
            None => branches.remove(name),
        };
        Self {
            branches: branches.into_iter().collect(),
            upstream: self.upstream,
        }
    }
}

impl Root for Repository {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6be3_7c5e_0a1d_4f6e_9c20_7265_706f_0001);

    fn references(&self) -> Vec<Uuid> {
        self.upstream.into_iter().collect()
    }
}

pub type RepositoryContent = Document<Repository>;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConflictKind {
    #[default]
    Content,
    Placement,
    Name,
    EditedAndRemoved,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Checkout {
    pub repository: Option<Uuid>,
    pub branch: String,
    pub base: Option<CommitId>,
    pub root: Option<Uuid>,
    pub clean: Map<Uuid, CommitId>,
    pub conflicts: List<CheckoutConflict>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct CheckoutConflict {
    pub block: Uuid,
    pub content_type: Uuid,
    pub kind: ConflictKind,
    pub base: Option<Uuid>,
    pub ours: Option<Uuid>,
    pub theirs: Option<Uuid>,
}

impl CheckoutConflict {
    pub fn versions(&self) -> impl Iterator<Item = Uuid> + '_ {
        [self.base, self.ours, self.theirs].into_iter().flatten()
    }
}

impl Checkout {
    pub fn is_clean_at(&self, block: Uuid, head: Option<CommitId>) -> bool {
        head.is_some() && self.clean.get(&block).copied() == head
    }
}

impl Root for Checkout {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6be3_7c5e_0a1d_4f6e_9c20_636b_6f75_0001);

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<Uuid> = self.repository.into_iter().chain(self.root).collect();
        references.dedup();
        references
    }
}

pub type CheckoutContent = Document<Checkout>;

pub fn scope_mask(checkout: Uuid) -> u128 {
    let mut mixed = checkout.as_u128() ^ 0x6be3_5c0e_0000_0000_9e37_79b9_7f4a_7c15;
    for _ in 0..3 {
        mixed ^= mixed >> 67;
        mixed = mixed.wrapping_mul(0x9e37_79b9_7f4a_7c15_f39c_c060_5ced_c835);
        mixed ^= mixed >> 59;
    }
    mixed | 1 << 64
}

pub fn masked(id: Uuid, mask: u128) -> Uuid {
    Uuid::from_u128(id.as_u128() ^ mask)
}

pub fn local_id(real: Uuid, metadata: &BlockMetadata, mask: u128) -> Uuid {
    match metadata.local_id {
        Some(local) if masked(local, mask) == real => local,
        _ => real,
    }
}
