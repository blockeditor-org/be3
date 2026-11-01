use std::time::Duration;

use be_commit::RetentionPolicy;
use be_commit::retention::DAY;
use be_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;
use crate::resolution::{Resolution, SettingsSource};
use crate::settings::{KindRule, Layered, union};

const SETTINGS_MASK: u128 = 0x6f72_6761_6e69_7a61_7469_6f6e_2d73_6574;

pub fn organization_settings(organization: Uuid) -> Uuid {
    Uuid::from_u128(organization.as_u128() ^ SETTINGS_MASK)
}

const MY_FILES_MASK: u128 = 0x6d79_2d66_696c_6573_2d6f_662d_6163_6374;

pub const fn my_files(organization: Uuid, account: Uuid) -> Uuid {
    Uuid::from_u128(organization.as_u128() ^ account.as_u128() ^ MY_FILES_MASK)
}

pub fn my_files_of(organization: Uuid, block: Uuid) -> Uuid {
    Uuid::from_u128(organization.as_u128() ^ block.as_u128() ^ MY_FILES_MASK)
}

pub const SYSTEM_TYPES: [Uuid; 2] = [
    crate::settings::Settings::CONTENT_TYPE,
    crate::settings::SettingsProfile::CONTENT_TYPE,
];

pub fn is_workspace_type(content_type: Uuid) -> bool {
    !SYSTEM_TYPES.contains(&content_type)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Allowance {
    #[default]
    Allowed,
    Blocked,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum WhoMay {
    #[default]
    Everyone,
    Administrators,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct OrganizationPolicies {
    pub outside_sharing: Option<Allowance>,
    pub workspace_creation: Option<WhoMay>,
    pub history_days: Option<u32>,
    pub storage_quota: Option<u64>,
}

impl OrganizationPolicies {
    pub fn shares_outside(&self) -> bool {
        self.outside_sharing.unwrap_or_default() == Allowance::Allowed
    }

    pub fn workspace_creation(&self) -> WhoMay {
        self.workspace_creation.unwrap_or_default()
    }

    pub fn history_kept_for(&self) -> i64 {
        i64::from(self.history_days.unwrap_or(0)) * DAY
    }

    pub fn retention(&self) -> RetentionPolicy {
        RetentionPolicy::default().keeping_all_within(self.history_kept_for())
    }

    pub fn quota(&self) -> Option<u64> {
        self.storage_quota.filter(|quota| *quota > 0)
    }

    pub fn set_outside_sharing(value: Option<Allowance>) -> Edit {
        Self::OUTSIDE_SHARING.set(ObjectId::ROOT, &value).into()
    }

    pub fn set_workspace_creation(value: Option<WhoMay>) -> Edit {
        Self::WORKSPACE_CREATION.set(ObjectId::ROOT, &value).into()
    }

    pub fn set_history_days(value: Option<u32>) -> Edit {
        Self::HISTORY_DAYS.set(ObjectId::ROOT, &value).into()
    }

    pub fn set_storage_quota(value: Option<u64>) -> Edit {
        Self::STORAGE_QUOTA.set(ObjectId::ROOT, &value).into()
    }
}

impl Root for OrganizationPolicies {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6f72_6761_6e69_7a61_7469_6f6e_2d70_6f6c);
}

impl Layered for OrganizationPolicies {
    const NAME: &'static str = "Organization";
    const RULE: KindRule = KindRule::Policy;

    fn over(&self, lower: &Self) -> Self {
        Self {
            outside_sharing: self.outside_sharing.or(lower.outside_sharing),
            workspace_creation: self.workspace_creation.or(lower.workspace_creation),
            history_days: self.history_days.or(lower.history_days),
            storage_quota: self.storage_quota.or(lower.storage_quota),
        }
    }
}

pub type OrganizationPoliciesContent = Document<OrganizationPolicies>;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum OfflinePolicy {
    Required,
    #[default]
    Allowed,
    Blocked,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum LostAccess {
    #[default]
    Keep,
    Delete,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct OfflineSettings {
    pub policy: Option<OfflinePolicy>,
    pub workspaces: Map<Uuid, bool>,
    pub on_lost_access: Option<LostAccess>,
    pub grace_days: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OfflineChoice {
    pub keep: bool,
    pub fixed: bool,
    pub on_lost_access: LostAccess,
    pub grace: Duration,
}

impl OfflineSettings {
    pub const DEFAULT_GRACE_DAYS: u32 = 7;

    pub fn set_policy(policy: Option<OfflinePolicy>) -> Edit {
        Self::POLICY.set(ObjectId::ROOT, &policy).into()
    }

    pub fn keep_workspace(workspace: Uuid, keep: Option<bool>) -> Edit {
        Self::WORKSPACES
            .put(ObjectId::ROOT, &workspace, keep.as_ref())
            .into()
    }

    pub fn set_on_lost_access(value: Option<LostAccess>) -> Edit {
        Self::ON_LOST_ACCESS.set(ObjectId::ROOT, &value).into()
    }

    pub fn set_grace_days(days: Option<u32>) -> Edit {
        Self::GRACE_DAYS.set(ObjectId::ROOT, &days).into()
    }

    pub fn choice(resolution: &Resolution<Self>, workspace: Uuid, personal: bool) -> OfflineChoice {
        let policy = organization(resolution, |layer| layer.policy);
        let own = resolution.value().workspaces.get(&workspace).copied();
        let (keep, fixed) = match policy.unwrap_or_default() {
            OfflinePolicy::Required => (true, true),
            OfflinePolicy::Blocked => (false, true),
            OfflinePolicy::Allowed => (own.unwrap_or(true), false),
        };
        let on_lost_access = match personal {
            true => LostAccess::Keep,
            false => organization(resolution, |layer| layer.on_lost_access).unwrap_or_default(),
        };
        let days =
            organization(resolution, |layer| layer.grace_days).unwrap_or(Self::DEFAULT_GRACE_DAYS);
        OfflineChoice {
            keep,
            fixed,
            on_lost_access,
            grace: Duration::from_secs(u64::from(days) * 24 * 60 * 60),
        }
    }
}

fn organization<V>(
    resolution: &Resolution<OfflineSettings>,
    get: impl Fn(&OfflineSettings) -> Option<V>,
) -> Option<V> {
    [
        SettingsSource::OrganizationPolicy,
        SettingsSource::OrganizationDefault,
    ]
    .into_iter()
    .find_map(|source| resolution.layer(source).and_then(&get))
}

impl Root for OfflineSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6f66_666c_696e_652d_7365_7474_696e_6773);
}

impl Layered for OfflineSettings {
    const NAME: &'static str = "Offline";

    fn over(&self, lower: &Self) -> Self {
        Self {
            policy: self.policy.or(lower.policy),
            workspaces: union(&self.workspaces, &lower.workspaces),
            on_lost_access: self.on_lost_access.or(lower.on_lost_access),
            grace_days: self.grace_days.or(lower.grace_days),
        }
    }
}

pub type OfflineSettingsContent = Document<OfflineSettings>;
