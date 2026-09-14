use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Checklist {
    items: Vec<ChecklistItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChecklistItem {
    pub id: Uuid,
    pub text: String,
    pub done: bool,
}

impl Checklist {
    pub fn new() -> Self {
        Self::default()
    }

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
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ChecklistOperation {
    Add { id: Uuid, text: String },
    SetText { id: Uuid, text: String },
    SetDone { id: Uuid, done: bool },
    Remove { id: Uuid },
    ClearDone,
}

impl ChecklistOperation {
    pub fn add(text: impl Into<String>) -> Self {
        Self::Add {
            id: Uuid::new_v4(),
            text: text.into(),
        }
    }
}

impl Block for Checklist {
    type Operation = ChecklistOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6368_6563_6b6c_6973_742d_626c_6f63_6b31);

    fn apply_operation(checklist: &mut Self, operation: &Self::Operation) {
        match operation {
            ChecklistOperation::Add { id, text } => {
                if checklist.items.iter().any(|item| item.id == *id) {
                    return;
                }
                checklist.items.push(ChecklistItem {
                    id: *id,
                    text: text.clone(),
                    done: false,
                });
            }
            ChecklistOperation::SetText { id, text } => {
                if let Some(item) = checklist.item_mut(*id) {
                    item.text.clone_from(text);
                }
            }
            ChecklistOperation::SetDone { id, done } => {
                if let Some(item) = checklist.item_mut(*id) {
                    item.done = *done;
                }
            }
            ChecklistOperation::Remove { id } => checklist.items.retain(|item| item.id != *id),
            ChecklistOperation::ClearDone => checklist.items.retain(|item| !item.done),
        }
    }
}

#[cfg(test)]
mod tests;
