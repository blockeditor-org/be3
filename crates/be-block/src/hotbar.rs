use std::collections::HashSet;

use be_model::{Anchor, Change, Document, Edit, Item, List, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ChildChange, Root};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SlotKind {
    Builtin { tool: String },
    Locked { name: String },
    Folder { name: String },
    Component { name: String, compiled: Uuid },
}

impl Default for SlotKind {
    fn default() -> Self {
        Self::Folder {
            name: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Hotbar {
    pub slots: List<HotbarSlot>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct HotbarSlot {
    pub kind: SlotKind,
    pub slots: List<HotbarSlot>,
}

impl HotbarSlot {
    pub fn component(name: impl Into<String>, compiled: Uuid) -> Self {
        Self {
            kind: SlotKind::Component {
                name: name.into(),
                compiled,
            },
            slots: List::default(),
        }
    }

    pub fn folder(name: impl Into<String>, slots: impl IntoIterator<Item = HotbarSlot>) -> Self {
        Self {
            kind: SlotKind::Folder { name: name.into() },
            slots: slots.into_iter().collect(),
        }
    }

    pub fn compiled(&self) -> Option<Uuid> {
        match &self.kind {
            SlotKind::Component { compiled, .. } => Some(*compiled),
            _ => None,
        }
    }
}

fn walk<'a>(slots: &'a [Item<HotbarSlot>], visit: &mut impl FnMut(&'a Item<HotbarSlot>)) {
    for slot in slots {
        visit(slot);
        walk(&slot.slots, visit);
    }
}

impl Hotbar {
    pub fn component_refs(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        let mut refs = Vec::new();
        walk(&self.slots, &mut |slot| {
            if let Some(compiled) = slot.compiled()
                && seen.insert(compiled)
            {
                refs.push(compiled);
            }
        });
        refs
    }

    fn pinning(&self, compiled: Uuid) -> Vec<&Item<HotbarSlot>> {
        let mut found = Vec::new();
        walk(&self.slots, &mut |slot| {
            if slot.compiled() == Some(compiled) {
                found.push(slot);
            }
        });
        found
    }

    pub fn replace_all(&self, slots: impl IntoIterator<Item = HotbarSlot>) -> Edit {
        let removed = self.slots.iter().map(|slot| Change::remove(slot.id));
        let added = slots
            .into_iter()
            .map(|slot| Self::SLOTS.insert(ObjectId::ROOT, Anchor::End, &slot).1);
        removed.chain(added).collect()
    }

    pub fn unpin(&self, compiled: Uuid) -> Edit {
        self.pinning(compiled)
            .into_iter()
            .map(|slot| Change::remove(slot.id))
            .collect()
    }
}

impl Root for Hotbar {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x686f_7462_6172_2d63_6f6e_7465_6e74_0001);

    fn references(&self) -> Vec<Uuid> {
        self.component_refs()
            .into_iter()
            .filter_map(|compiled| Some(compiled))
            .collect()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(_) => None,
            ChildChange::Delete(old) => Some(self.unpin(old)),
            ChildChange::Replace { old, new } => Some(
                self.pinning(old)
                    .into_iter()
                    .filter_map(|slot| match &slot.kind {
                        SlotKind::Component { name, .. } => Some(HotbarSlot::KIND.set(
                            slot.id,
                            &SlotKind::Component {
                                name: name.clone(),
                                compiled: new,
                            },
                        )),
                        _ => None,
                    })
                    .collect(),
            ),
        }
    }
}

pub type HotbarContent = Document<Hotbar>;
