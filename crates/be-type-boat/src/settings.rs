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
    /// The terminals this account connected to sandboxes, keyed by terminal block id, with
    /// the credentials of each one's agent: `be` signs in only with its recovery phrase, and a
    /// rotation answers only a new token, so they are kept to write again after a resume.
    pub terminals: Map<String, TerminalAgent>,
}

/// A terminal host's agent in a sandbox (guides/terminal.md): the agent, the credentials
/// handed to the sandbox (`BlockAgent::credentials`, its token the latest), and where it runs.
#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalAgent {
    pub agent: String,
    pub credentials: String,
    pub sandbox: String,
    /// The boat.dev organization whose key reaches the sandbox, as `Boat::organization`.
    pub organization: String,
}

/// Leaves the credentials out, so they never reach a log.
impl std::fmt::Debug for TerminalAgent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalAgent")
            .field("agent", &self.agent)
            .field("sandbox", &self.sandbox)
            .field("organization", &self.organization)
            .finish_non_exhaustive()
    }
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

    pub fn terminal(&self, block: uuid::Uuid) -> Option<&TerminalAgent> {
        self.terminals.get(&block.to_string())
    }

    pub fn set_terminal(block: uuid::Uuid, terminal: Option<&TerminalAgent>) -> Edit {
        Self::TERMINALS
            .put(ObjectId::ROOT, &block.to_string(), terminal)
            .into()
    }
}

impl Layered for BoatSettings {
    const NAME: &'static str = "boat.dev";
    const VISIBILITY: KindVisibility = KindVisibility::Owner;

    fn over(&self, lower: &Self) -> Self {
        Self {
            keys: union(&self.keys, &lower.keys),
            terminals: union(&self.terminals, &lower.terminals),
        }
    }
}

impl Root for BoatSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x626f_6174_2d73_6574_7469_6e67_7300_0001);
}

pub type BoatSettingsContent = Document<BoatSettings>;
