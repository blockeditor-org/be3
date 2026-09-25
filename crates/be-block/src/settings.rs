use be_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ChildChange, Root};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ActivationCondition {
    Fallback,
    Client(Uuid),
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Settings {
    pub entries: Map<(Uuid, ActivationCondition), Uuid>,
}

impl Settings {
    pub fn resolve(&self, block_type: Uuid, client: Uuid) -> Option<Uuid> {
        self.entries
            .get(&(block_type, ActivationCondition::Client(client)))
            .or_else(|| {
                self.entries
                    .get(&(block_type, ActivationCondition::Fallback))
            })
            .copied()
    }

    pub fn set_entry(block_type: Uuid, activation: ActivationCondition, block: Uuid) -> Edit {
        Self::ENTRIES
            .put(ObjectId::ROOT, &(block_type, activation), Some(&block))
            .into()
    }
}

impl Root for Settings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7365_7474_696e_6773_2d62_6c6f_636b_3031);

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<Uuid> = self.entries.values().copied().collect();
        references.sort_unstable();
        references.dedup();
        references
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(_) => None,
            ChildChange::Delete(old) => Some(
                self.entries
                    .iter()
                    .filter(|(_, block)| **block == old)
                    .map(|(key, _)| Self::ENTRIES.put(ObjectId::ROOT, key, None))
                    .collect(),
            ),
            ChildChange::Replace { old, new } => Some(
                self.entries
                    .iter()
                    .filter(|(_, block)| **block == old)
                    .map(|(key, _)| Self::ENTRIES.put(ObjectId::ROOT, key, Some(&new)))
                    .collect(),
            ),
        }
    }
}

pub type SettingsContent = Document<Settings>;
