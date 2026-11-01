use std::collections::{BTreeMap, BTreeSet};

use be_commit::CommitId;
use be_model::{Document, List, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BlockMetadata, Root};

pub const MAIN_BRANCH: &str = "main";

pub const OPERATION_LIMIT: usize = 200;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Repository {
    pub branches: Map<String, CommitId>,
    pub upstream: Option<Uuid>,
    pub operations: List<Operation>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Operation {
    pub id: Uuid,
    pub time: i64,
    pub author: Uuid,
    pub action: Action,
    pub checkout: Option<Uuid>,
    pub before: View,
    pub after: View,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct View {
    pub branches: BTreeMap<String, CommitId>,
    pub working: Option<CommitId>,
    pub branch: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Action {
    #[default]
    Other,
    StartedVersioning,
    Finished {
        #[serde(default)]
        description: String,
    },
    BroughtIn {
        #[serde(default)]
        branch: String,
    },
    Switched {
        #[serde(default)]
        branch: String,
    },
    MadeBranch {
        #[serde(default)]
        name: String,
    },
    Merged {
        #[serde(default)]
        branch: String,
    },
    Resolved {
        #[serde(default)]
        name: String,
        #[serde(default)]
        kept: Kept,
    },
    NewCheckout {
        #[serde(default)]
        branch: String,
    },
    Pulled,
    Pushed,
    Undid {
        #[serde(default)]
        operation: Uuid,
        #[serde(default)]
        what: String,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Kept {
    Base,
    Ours,
    Theirs,
    #[default]
    AsItIs,
}

impl Action {
    pub fn describe(&self) -> String {
        match self {
            Self::Other => "Changed the history".to_owned(),
            Self::StartedVersioning => "Started keeping versions".to_owned(),
            Self::Finished { description } => {
                format!("Saved the change \u{201c}{description}\u{201d}")
            }
            Self::BroughtIn { branch } => format!("Brought in newer changes from {branch}"),
            Self::Switched { branch } => format!("Moved to {branch}"),
            Self::MadeBranch { name } => format!("Made the branch {name}"),
            Self::Merged { branch } => format!("Combined {branch} into this one"),
            Self::Resolved { name, kept } => match kept {
                Kept::Base => format!("Went back to the older version of {name}"),
                Kept::Ours => format!("Kept this side's version of {name}"),
                Kept::Theirs => format!("Took the incoming version of {name}"),
                Kept::AsItIs => format!("Marked {name} as sorted out"),
            },
            Self::NewCheckout { branch } => format!("Made a copy to edit {branch} in"),
            Self::Pulled => "Brought in changes from the original".to_owned(),
            Self::Pushed => "Sent changes to the original".to_owned(),
            Self::Undid { what, .. } => format!("Undid: {what}"),
        }
    }
}

impl Repository {
    pub fn branch(&self, name: &str) -> Option<CommitId> {
        self.branches.get(name).copied()
    }

    pub fn with_branch(&self, name: &str, head: Option<CommitId>) -> Self {
        let mut branches: BTreeMap<String, CommitId> = (*self.branches).clone();
        match head {
            Some(head) => branches.insert(name.to_owned(), head),
            None => branches.remove(name),
        };
        Self {
            branches: branches.into_iter().collect(),
            ..self.clone()
        }
    }

    pub fn view(&self, checkout: Option<&Checkout>) -> View {
        View {
            branches: (*self.branches).clone(),
            working: checkout.and_then(|checkout| checkout.working),
            branch: checkout
                .map(|checkout| checkout.branch.clone())
                .unwrap_or_default(),
        }
    }

    pub fn operation(&self, id: Uuid) -> Option<&Operation> {
        self.operations
            .iter()
            .map(|item| &item.value)
            .find(|operation| operation.id == id)
    }

    pub fn record(&mut self, operation: Operation) {
        let skip = (self.operations.len() + 1).saturating_sub(OPERATION_LIMIT);
        let kept = self
            .operations
            .iter()
            .skip(skip)
            .map(|item| (item.id, item.value.clone()));
        let added = std::iter::once((ObjectId::from_uuid(operation.id), operation));
        self.operations = List::with_ids(kept.chain(added).collect::<Vec<_>>());
    }

    pub fn undone(&self) -> BTreeSet<Uuid> {
        let mut undone = BTreeSet::new();
        for operation in self.operations.iter().rev().map(|item| &item.value) {
            if let Action::Undid {
                operation: target, ..
            } = &operation.action
                && !undone.contains(&operation.id)
            {
                undone.insert(*target);
            }
        }
        undone
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
    Metadata,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Checkout {
    pub repository: Option<Uuid>,
    pub branch: String,
    pub working: Option<CommitId>,
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

impl Root for Checkout {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6be3_7c5e_0a1d_4f6e_9c20_636b_6f75_0001);

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<Uuid> = self.repository.into_iter().chain(self.root).collect();
        references.extend(
            self.conflicts
                .iter()
                .flat_map(|conflict| conflict.versions()),
        );
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
    match metadata.get(&crate::LOCAL_ID) {
        Some(local) if masked(local, mask) == real => local,
        _ => real,
    }
}
