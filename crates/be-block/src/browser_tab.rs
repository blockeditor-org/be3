use be_model::{Anchor, Change, Document, Edit, Item, List, Model, ObjectId};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct BrowserTab {
    pub history: List<HistoryItem>,
    pub current: Option<ObjectId>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct HistoryItem {
    pub url: String,
    pub title: String,
}

impl HistoryItem {
    pub fn new(url: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            title: title.into(),
        }
    }

    fn blank() -> Self {
        Self::new("about:blank", "")
    }
}

impl BrowserTab {
    pub fn index(&self) -> usize {
        self.current_item()
            .and_then(|current| self.history.iter().position(|item| item.id == current.id))
            .unwrap_or(0)
    }

    pub fn current(&self) -> HistoryItem {
        self.current_item()
            .map(|item| item.value.clone())
            .unwrap_or_else(HistoryItem::blank)
    }

    pub fn can_go_forward(&self) -> bool {
        self.index() + 1 < self.history.len()
    }

    fn current_item(&self) -> Option<&Item<HistoryItem>> {
        self.current
            .and_then(|current| self.history.get(current))
            .or_else(|| self.history.last())
    }

    pub fn push(&self, item: &HistoryItem) -> Edit {
        let mut changes: Vec<Change> = self
            .history
            .iter()
            .skip(self.index() + 1)
            .map(|later| Change::remove(later.id))
            .collect();
        let anchor = self
            .current_item()
            .map_or(Anchor::End, |current| Anchor::After(current.id));
        let (id, insert) = Self::HISTORY.insert(ObjectId::ROOT, anchor, item);
        changes.push(insert);
        changes.push(Self::CURRENT.set(ObjectId::ROOT, &Some(id)));
        Edit(changes)
    }

    pub fn replace(&self, item: &HistoryItem) -> Edit {
        let Some(current) = self.current_item() else {
            return self.push(item);
        };
        let mut changes = Vec::new();
        if current.url != item.url {
            changes.push(HistoryItem::URL.set(current.id, &item.url));
        }
        if current.title != item.title {
            changes.push(HistoryItem::TITLE.set(current.id, &item.title));
        }
        Edit(changes)
    }

    pub fn go(&self, index: usize) -> Edit {
        match self.history.get_index(index) {
            Some(item) => Self::CURRENT.set(ObjectId::ROOT, &Some(item.id)).into(),
            None => Edit::default(),
        }
    }
}

impl Root for BrowserTab {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7765_622d_6272_6f77_7365_722d_7461_6203);

    fn name(&self) -> Option<String> {
        let title = self.current().title.trim().to_owned();
        (!title.is_empty()).then_some(title)
    }
}

pub type BrowserTabContent = Document<BrowserTab>;
