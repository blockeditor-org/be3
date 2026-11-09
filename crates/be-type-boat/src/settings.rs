use be_block::Root;
use be_block::settings::{KindVisibility, Layered, union};
use be_block_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The boat.dev keys of the account that made it, one per boat account or organization. An
/// owner-only kind: no one else reads it, administrators and the server included
/// (guides/settings.md, Owner only; decisions/an_app_owns_its_credentials_for_outside_services.md).
#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct BoatSettings {
    /// Keyed by `Boat::organization`, empty for a key of the person's own account.
    pub keys: Map<String, BoatKey>,
}

/// A boat.dev API key and what the person said about it when they pasted it.
#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct BoatKey {
    pub secret: String,
    /// When the key expires, in seconds since the Unix epoch, as the person entered it:
    /// boat's `GET /me` doesn't say.
    pub expires: i64,
    /// Whose key it is, as `GET /me` answered when it was checked.
    pub login: String,
}

/// Leaves the secret out, so a key never reaches a log.
impl std::fmt::Debug for BoatKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoatKey")
            .field("expires", &self.expires)
            .field("login", &self.login)
            .finish_non_exhaustive()
    }
}

impl BoatSettings {
    pub fn key(&self, organization: &str) -> Option<&BoatKey> {
        self.keys.get(organization)
    }

    pub fn set_key(organization: &str, key: Option<&BoatKey>) -> Edit {
        Self::KEYS
            .put(ObjectId::ROOT, &organization.to_owned(), key)
            .into()
    }
}

impl Layered for BoatSettings {
    const NAME: &'static str = "boat.dev";
    const VISIBILITY: KindVisibility = KindVisibility::Owner;

    fn over(&self, lower: &Self) -> Self {
        Self {
            keys: union(&self.keys, &lower.keys),
        }
    }
}

impl Root for BoatSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x626f_6174_2d73_6574_7469_6e67_7300_0001);
}

pub type BoatSettingsContent = Document<BoatSettings>;
