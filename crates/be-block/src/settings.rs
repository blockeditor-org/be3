use be_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ChildChange, Root, WORKSPACE_EDITOR};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ActivationCondition {
    Fallback,
    Client(Uuid),
    Desktop(Uuid),
}

impl ActivationCondition {
    pub fn session(client: Uuid, desktop: bool) -> Self {
        match desktop {
            true => Self::Desktop(client),
            false => Self::Client(client),
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Settings {
    pub entries: Map<(Uuid, ActivationCondition), Uuid>,
    pub profiles: Map<Uuid, ()>,
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

    pub fn profile(&self, client: Uuid, desktop: bool) -> Option<Uuid> {
        self.entries
            .get(&(
                WORKSPACE_EDITOR,
                ActivationCondition::session(client, desktop),
            ))
            .copied()
            .filter(|profile| self.profiles.contains_key(profile))
    }

    pub fn profiles(&self) -> Vec<Uuid> {
        self.profiles.keys().copied().collect()
    }

    pub fn add_profile(client: Uuid, desktop: bool, profile: Uuid) -> Edit {
        std::iter::once(Self::PROFILES.put(ObjectId::ROOT, &profile, Some(&())))
            .chain(Self::use_profile(client, desktop, profile).0)
            .collect()
    }

    pub fn use_profile(client: Uuid, desktop: bool, profile: Uuid) -> Edit {
        Self::set_entry(
            WORKSPACE_EDITOR,
            ActivationCondition::session(client, desktop),
            profile,
        )
    }
}

impl Root for Settings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7365_7474_696e_6773_2d62_6c6f_636b_3031);

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<Uuid> = self
            .entries
            .values()
            .chain(self.profiles.keys())
            .copied()
            .collect();
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
                    .chain(
                        self.profiles
                            .contains_key(&old)
                            .then(|| Self::PROFILES.put(ObjectId::ROOT, &old, None)),
                    )
                    .collect(),
            ),
            ChildChange::Replace { old, new } => Some(
                self.entries
                    .iter()
                    .filter(|(_, block)| **block == old)
                    .map(|(key, _)| Self::ENTRIES.put(ObjectId::ROOT, key, Some(&new)))
                    .chain(
                        self.profiles
                            .contains_key(&old)
                            .then(|| Self::PROFILES.put(ObjectId::ROOT, &old, None)),
                    )
                    .chain(
                        self.profiles
                            .contains_key(&old)
                            .then(|| Self::PROFILES.put(ObjectId::ROOT, &new, Some(&()))),
                    )
                    .collect(),
            ),
        }
    }
}

pub type SettingsContent = Document<Settings>;
