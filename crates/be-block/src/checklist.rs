use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Checklist {
    pub items: List<ChecklistItem>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct ChecklistItem {
    pub text: String,
    pub done: bool,
}

impl Checklist {
    pub fn done_count(&self) -> usize {
        self.items.iter().filter(|item| item.done).count()
    }

    pub fn add(text: impl Into<String>) -> (ObjectId, Edit) {
        let item = ChecklistItem {
            text: text.into(),
            done: false,
        };
        let (id, change) = Self::ITEMS.insert(ObjectId::ROOT, Anchor::End, &item);
        (id, change.into())
    }

    pub fn set_text(item: ObjectId, text: impl Into<String>) -> Edit {
        ChecklistItem::TEXT.set(item, &text.into()).into()
    }

    pub fn set_done(item: ObjectId, done: bool) -> Edit {
        ChecklistItem::DONE.set(item, &done).into()
    }

    pub fn remove(item: ObjectId) -> Edit {
        Change::remove(item).into()
    }

    pub fn clear_done(&self) -> Edit {
        self.items
            .iter()
            .filter(|item| item.done)
            .map(|item| Change::remove(item.id))
            .collect()
    }
}

impl Root for Checklist {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6368_6563_6b6c_6973_742d_626c_6f63_6b31);
}

pub type ChecklistContent = Document<Checklist>;
