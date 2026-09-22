use std::collections::{BTreeMap, BTreeSet};

use be_commit::{MergeResult, merge_slices};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BlockContent, ContentError, LiveEdit, Merge};

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
        let base_items = by_id(base);
        let ours_items = by_id(ours);
        let theirs_items = by_id(theirs);
        let mut conflicts = 0;
        let mut kept = BTreeMap::new();
        let ids: BTreeSet<Uuid> = base_items
            .keys()
            .chain(ours_items.keys())
            .chain(theirs_items.keys())
            .copied()
            .collect();
        for id in ids {
            let resolved = merge_item(
                base_items.get(&id).copied(),
                ours_items.get(&id).copied(),
                theirs_items.get(&id).copied(),
                &mut conflicts,
            );
            if let Some(item) = resolved {
                kept.insert(id, item);
            }
        }

        let order = merge_order(base, ours, theirs);
        let mut items = Vec::with_capacity(kept.len());
        for id in order {
            if let Some(item) = kept.remove(&id) {
                items.push(item);
            }
        }
        items.extend(kept.into_values());

        let merged = Self { items };
        match conflicts {
            0 => MergeResult::Clean(merged),
            conflicts => MergeResult::Conflicted {
                value: merged,
                conflicts,
            },
        }
    }
}

fn by_id(checklist: &ChecklistContent) -> BTreeMap<Uuid, &ChecklistItem> {
    checklist.items.iter().map(|item| (item.id, item)).collect()
}

fn ids(checklist: &ChecklistContent) -> Vec<Uuid> {
    checklist.items.iter().map(|item| item.id).collect()
}

fn merge_order(
    base: &ChecklistContent,
    ours: &ChecklistContent,
    theirs: &ChecklistContent,
) -> Vec<Uuid> {
    let outcome = merge_slices(&ids(base), &ids(ours), &ids(theirs));
    let mut order = outcome.merged;
    for conflict in outcome.conflicts.iter().rev() {
        let after_ours = conflict.at + conflict.ours.len();
        let only_theirs = conflict
            .theirs
            .iter()
            .filter(|id| !conflict.ours.contains(id))
            .copied();
        order.splice(after_ours..after_ours, only_theirs);
    }
    order
}

fn merge_item(
    base: Option<&ChecklistItem>,
    ours: Option<&ChecklistItem>,
    theirs: Option<&ChecklistItem>,
    conflicts: &mut usize,
) -> Option<ChecklistItem> {
    if ours == base {
        return theirs.cloned();
    }
    if theirs == base || theirs == ours {
        return ours.cloned();
    }
    match (base, ours, theirs) {
        (Some(base), Some(ours), Some(theirs)) => Some(ChecklistItem {
            id: base.id,
            text: pick(&base.text, &ours.text, &theirs.text, conflicts).clone(),
            done: *pick(&base.done, &ours.done, &theirs.done, conflicts),
        }),
        (_, Some(ours), _) => {
            *conflicts += 1;
            Some(ours.clone())
        }
        (_, None, theirs) => {
            *conflicts += 1;
            theirs.cloned()
        }
    }
}

fn pick<'a, T: PartialEq>(base: &'a T, ours: &'a T, theirs: &'a T, conflicts: &mut usize) -> &'a T {
    if ours == base {
        theirs
    } else if theirs == base || theirs == ours {
        ours
    } else {
        *conflicts += 1;
        ours
    }
}
