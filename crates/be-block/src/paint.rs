use be_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::blob::{Blob, BlobKind};
use crate::{ChildChange, Root};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PaintSnapshotHeader {
    pub path: String,
    pub hash: String,
}

pub struct PaintSnapshotFile;

impl BlobKind for PaintSnapshotFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7061_696e_742d_736e_6170_2d63_6f6e_0002);

    type Header = PaintSnapshotHeader;

    fn name(header: &PaintSnapshotHeader) -> &str {
        header.path.rsplit('/').next().unwrap_or(&header.path)
    }
}

pub type PaintSnapshotContent = Blob<PaintSnapshotFile>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Approval {
    pub hash: String,
    pub snapshot: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovedPainting {
    pub path: String,
    pub hash: String,
    pub snapshot: Uuid,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct PaintReview {
    pub approvals: Map<String, Approval>,
}

impl PaintReview {
    pub fn approved(&self) -> Vec<ApprovedPainting> {
        self.approvals
            .iter()
            .map(|(path, approval)| ApprovedPainting {
                path: path.clone(),
                hash: approval.hash.clone(),
                snapshot: approval.snapshot,
            })
            .collect()
    }

    pub fn approval(&self, path: &str) -> Option<&Approval> {
        self.approvals.get(path)
    }

    pub fn approve(path: &str, hash: impl Into<String>, snapshot: Uuid) -> Edit {
        let approval = Approval {
            hash: hash.into(),
            snapshot,
        };
        Self::APPROVALS
            .put(ObjectId::ROOT, &path.to_owned(), Some(&approval))
            .into()
    }

    pub fn forget(path: &str) -> Edit {
        Self::APPROVALS
            .put(ObjectId::ROOT, &path.to_owned(), None)
            .into()
    }
}

impl Root for PaintReview {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7061_696e_742d_7265_7669_6577_2d63_0002);

    fn references(&self) -> Vec<Uuid> {
        self.approvals
            .values()
            .filter_map(|approval| Some(approval.snapshot))
            .collect()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let (old, new) = match change {
            ChildChange::Add(_) => return None,
            ChildChange::Delete(old) => (old, None),
            ChildChange::Replace { old, new } => (old, Some(new)),
        };
        Some(
            self.approvals
                .iter()
                .filter(|(_, approval)| approval.snapshot == old)
                .map(|(path, approval)| {
                    let repointed = new.map(|snapshot| Approval {
                        snapshot,
                        ..approval.clone()
                    });
                    Self::APPROVALS.put(ObjectId::ROOT, path, repointed.as_ref())
                })
                .collect(),
        )
    }
}

pub type PaintReviewContent = Document<PaintReview>;
