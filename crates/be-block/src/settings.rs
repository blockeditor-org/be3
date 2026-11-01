use be_model::{Change, Document, Edit, Map, Model, ObjectId};
use uuid::Uuid;

use crate::device_class::DeviceClass;
use crate::{ChildChange, Root};

pub use crate::resolution::{Resolution, ResolutionLayers, SettingsSource};

pub trait Layered: Root + Clone + Default {
    const NAME: &'static str;
    const FOLLOWS: Follows = Follows::Viewer;
    const RULE: KindRule = KindRule::Default;
    const VISIBILITY: KindVisibility = KindVisibility::Members;

    fn over(&self, lower: &Self) -> Self;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Follows {
    Block,
    Viewer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KindRule {
    Default,
    Policy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum KindVisibility {
    Inside,
    #[default]
    Members,
}

pub fn union<K: Ord + Clone, V: Clone>(upper: &Map<K, V>, lower: &Map<K, V>) -> Map<K, V> {
    lower
        .iter()
        .chain(upper.iter())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub fn layered<R: Layered>(layers: impl IntoIterator<Item = R>) -> R {
    layers
        .into_iter()
        .fold(R::default(), |lower, upper| upper.over(&lower))
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Settings {
    pub kinds: Map<Uuid, Uuid>,
    pub profiles: Map<Uuid, ()>,
    pub sessions: Map<Uuid, ()>,
    pub device_profiles: Map<Uuid, Uuid>,
    pub device_sessions: Map<(Uuid, Uuid), Uuid>,
    pub policies: Map<Uuid, Uuid>,
}

impl Settings {
    pub fn merged(held: impl IntoIterator<Item = Self>) -> Self {
        held.into_iter()
            .reduce(|upper, lower| Self {
                kinds: union(&upper.kinds, &lower.kinds),
                profiles: union(&upper.profiles, &lower.profiles),
                sessions: union(&upper.sessions, &lower.sessions),
                device_profiles: union(&upper.device_profiles, &lower.device_profiles),
                device_sessions: union(&upper.device_sessions, &lower.device_sessions),
                policies: union(&upper.policies, &lower.policies),
            })
            .unwrap_or_default()
    }

    pub fn kind(&self, kind: Uuid) -> Option<Uuid> {
        self.kinds.get(&kind).copied()
    }

    pub fn set_kind(kind: Uuid, block: Uuid) -> Edit {
        Self::KINDS.put(ObjectId::ROOT, &kind, Some(&block)).into()
    }

    pub fn policy(&self, kind: Uuid) -> Option<Uuid> {
        self.policies.get(&kind).copied()
    }

    pub fn set_policy(kind: Uuid, block: Uuid) -> Edit {
        Self::POLICIES
            .put(ObjectId::ROOT, &kind, Some(&block))
            .into()
    }

    pub fn layers(&self, kind: Uuid, profile: Option<&SettingsProfile>) -> crate::Layers {
        crate::Layers {
            every_device: self.kind(kind),
            this_device: profile.and_then(|profile| profile.kind(kind)),
        }
    }

    pub fn profile(&self, client: Uuid) -> Option<Uuid> {
        self.device_profiles
            .get(&client)
            .copied()
            .filter(|profile| self.profiles.contains_key(profile))
    }

    pub fn profiles(&self) -> Vec<Uuid> {
        self.profiles.keys().copied().collect()
    }

    pub fn add_profile(client: Uuid, profile: Uuid) -> Edit {
        [
            Self::PROFILES.put(ObjectId::ROOT, &profile, Some(&())),
            Self::DEVICE_PROFILES.put(ObjectId::ROOT, &client, Some(&profile)),
        ]
        .into_iter()
        .collect()
    }

    pub fn list_profile(profile: Uuid) -> Edit {
        Self::PROFILES
            .put(ObjectId::ROOT, &profile, Some(&()))
            .into()
    }

    pub fn use_profile(client: Uuid, profile: Uuid) -> Edit {
        Self::DEVICE_PROFILES
            .put(ObjectId::ROOT, &client, Some(&profile))
            .into()
    }

    pub fn session(&self, shell: Uuid, client: Uuid) -> Option<Uuid> {
        self.device_sessions
            .get(&(client, shell))
            .copied()
            .filter(|session| self.sessions.contains_key(session))
    }

    pub fn sessions(&self) -> Vec<Uuid> {
        self.sessions.keys().copied().collect()
    }

    pub fn add_session(shell: Uuid, client: Uuid, session: Uuid) -> Edit {
        [
            Self::SESSIONS.put(ObjectId::ROOT, &session, Some(&())),
            Self::DEVICE_SESSIONS.put(ObjectId::ROOT, &(client, shell), Some(&session)),
        ]
        .into_iter()
        .collect()
    }

    pub fn list_session(session: Uuid) -> Edit {
        Self::SESSIONS
            .put(ObjectId::ROOT, &session, Some(&()))
            .into()
    }

    pub fn use_session(shell: Uuid, client: Uuid, session: Uuid) -> Edit {
        Self::DEVICE_SESSIONS
            .put(ObjectId::ROOT, &(client, shell), Some(&session))
            .into()
    }

    fn repoint(&self, old: Uuid, new: Option<Uuid>) -> Edit {
        let mut changes: Vec<Change> = Vec::new();
        changes.extend(
            self.kinds
                .iter()
                .filter(|(_, block)| **block == old)
                .map(|(kind, _)| Self::KINDS.put(ObjectId::ROOT, kind, new.as_ref())),
        );
        changes.extend(
            self.policies
                .iter()
                .filter(|(_, block)| **block == old)
                .map(|(kind, _)| Self::POLICIES.put(ObjectId::ROOT, kind, new.as_ref())),
        );
        changes.extend(
            self.device_profiles
                .iter()
                .filter(|(_, profile)| **profile == old)
                .map(|(client, _)| Self::DEVICE_PROFILES.put(ObjectId::ROOT, client, new.as_ref())),
        );
        changes.extend(
            self.device_sessions
                .iter()
                .filter(|(_, session)| **session == old)
                .map(|(key, _)| Self::DEVICE_SESSIONS.put(ObjectId::ROOT, key, new.as_ref())),
        );
        for (listed, field) in [
            (&self.profiles, Self::PROFILES),
            (&self.sessions, Self::SESSIONS),
        ] {
            if listed.contains_key(&old) {
                changes.push(field.put(ObjectId::ROOT, &old, None));
                changes.extend(new.map(|new| field.put(ObjectId::ROOT, &new, Some(&()))));
            }
        }
        changes.into_iter().collect()
    }
}

impl Root for Settings {
    const CONTENT_TYPE: Uuid = be_graph::SETTINGS_TYPE;

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<Uuid> = self
            .kinds
            .values()
            .chain(self.policies.values())
            .chain(self.profiles.keys())
            .chain(self.sessions.keys())
            .copied()
            .collect();
        references.sort_unstable();
        references.dedup();
        references
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(_) => None,
            ChildChange::Delete(old) => Some(self.repoint(old, None)),
            ChildChange::Replace { old, new } => Some(self.repoint(old, Some(new))),
        }
    }
}

pub type SettingsContent = Document<Settings>;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct SettingsProfile {
    pub kinds: Map<Uuid, Uuid>,
    pub class: Option<DeviceClass>,
}

impl SettingsProfile {
    pub fn set_class(class: Option<DeviceClass>) -> Edit {
        Self::CLASS.set(ObjectId::ROOT, &class).into()
    }

    pub fn kind(&self, kind: Uuid) -> Option<Uuid> {
        self.kinds.get(&kind).copied()
    }

    pub fn set_kind(kind: Uuid, block: Uuid) -> Edit {
        Self::KINDS.put(ObjectId::ROOT, &kind, Some(&block)).into()
    }
}

impl Root for SettingsProfile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7365_7474_696e_6773_2d70_726f_6669_6c65);

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<Uuid> = self.kinds.values().copied().collect();
        references.sort_unstable();
        references.dedup();
        references
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let (old, new) = match change {
            ChildChange::Add(_) => return None,
            ChildChange::Delete(old) => (old, None),
            ChildChange::Replace { old, new } => (old, Some(new)),
        };
        Some(
            self.kinds
                .iter()
                .filter(|(_, block)| **block == old)
                .map(|(kind, _)| Self::KINDS.put(ObjectId::ROOT, kind, new.as_ref()))
                .collect(),
        )
    }
}

pub type SettingsProfileContent = Document<SettingsProfile>;
