use be_commit::MergeResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    BlockContent, ContentError, LiveEdit, Merge,
    keyed::{merge_keyed, pick},
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChecklistContent {
    items: Vec<ChecklistItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChecklistItem {
    pub id: Uuid,
    pub text: String,
    pub done: bool,
}

impl ChecklistContent {
    pub fn items(&self) -> &[ChecklistItem] {
        &self.items
    }

    pub fn done_count(&self) -> usize {
        self.items.iter().filter(|item| item.done).count()
    }

    fn item_mut(&mut self, id: Uuid) -> Option<&mut ChecklistItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChecklistOp {
    Add { id: Uuid, text: String },
    SetText { id: Uuid, text: String },
    SetDone { id: Uuid, done: bool },
    Remove { id: Uuid },
    ClearDone,
}

impl ChecklistOp {
    pub fn add(text: impl Into<String>) -> Self {
        Self::Add {
            id: Uuid::new_v4(),
            text: text.into(),
        }
    }
}

impl BlockContent for ChecklistContent {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6368_6563_6b6c_6973_742d_626c_6f63_6b02);

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("checklist"))
    }
}

impl LiveEdit for ChecklistContent {
    type Op = ChecklistOp;

    fn apply(&mut self, operation: &Self::Op) {
        match operation {
            ChecklistOp::Add { id, text } => {
                if self.items.iter().any(|item| item.id == *id) {
                    return;
                }
                self.items.push(ChecklistItem {
                    id: *id,
                    text: text.clone(),
                    done: false,
                });
            }
            ChecklistOp::SetText { id, text } => {
                if let Some(item) = self.item_mut(*id) {
                    item.text.clone_from(text);
                }
            }
            ChecklistOp::SetDone { id, done } => {
                if let Some(item) = self.item_mut(*id) {
                    item.done = *done;
                }
            }
            ChecklistOp::Remove { id } => self.items.retain(|item| item.id != *id),
            ChecklistOp::ClearDone => self.items.retain(|item| !item.done),
        }
    }
}

impl Merge for ChecklistContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        let merged = merge_keyed(
            &base.items,
            &ours.items,
            &theirs.items,
            |item| item.id,
            |base, ours, theirs, conflicts| ChecklistItem {
                id: base.id,
                text: pick(&base.text, &ours.text, &theirs.text, conflicts),
                done: pick(&base.done, &ours.done, &theirs.done, conflicts),
            },
        );
        match merged {
            MergeResult::Clean(items) => MergeResult::Clean(Self { items }),
            MergeResult::Conflicted { value, conflicts } => MergeResult::Conflicted {
                value: Self { items: value },
                conflicts,
            },
        }
    }
}
