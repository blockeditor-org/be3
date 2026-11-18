//! The host state as the host reads it: a copy held in memory (`Held`), read without a file,
//! and changed by `Write`s that go to it at once and to its keeper (`kept::Keeper`). A native
//! host keeps its keeper here and writes through it as it changes the copy; the web's page
//! hands each write to a worker of the host's own that holds the keeper, so it makes no file
//! call (decisions/web_host_state_in_a_worker.md). Either way there is one copy the host reads
//! and one file it is kept in.

use std::{cell::RefCell, collections::BTreeMap, path::Path};

use be_block_protocol::Organization;
use uuid::Uuid;

use super::{
    HostStateError, SavedAccount,
    kept::{ACTIVE, Keeper, Write, organization_key},
};

/// What `host.sqlite3` holds that the host reads: its saved accounts (in the order they are
/// listed: by name, ignoring ASCII case, then id), its settings, identity secrets and the
/// input, display and remapper settings.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct Held {
    pub(super) accounts: Vec<SavedAccount>,
    pub(super) settings: BTreeMap<String, String>,
    pub(super) identities: BTreeMap<(String, Uuid), [u8; 32]>,
    pub(super) input_settings: Option<Vec<u8>>,
    pub(super) display_settings: Option<Vec<u8>>,
    pub(super) remapper: Option<Vec<u8>>,
}

impl Held {
    pub fn accounts(&self) -> &[SavedAccount] {
        &self.accounts
    }

    fn apply(&mut self, write: &Write) {
        match write {
            Write::SaveAccount(saved) => {
                match self.accounts.iter_mut().find(|held| same(held, saved)) {
                    Some(held) => *held = saved.clone(),
                    None => self.accounts.push(saved.clone()),
                }
                self.accounts.sort_by_key(|account| {
                    (account.name.to_ascii_lowercase(), account.id.to_string())
                });
            }
            Write::RemoveAccount(saved) => {
                self.accounts.retain(|held| !same(held, saved));
                self.identities
                    .remove(&(saved.server.key().to_owned(), saved.id));
                self.settings.remove(&organization_key(saved));
                let active = (
                    self.settings.get(ACTIVE[0]).map(String::as_str),
                    self.settings.get(ACTIVE[1]).map(String::as_str),
                );
                if active
                    == (
                        Some(saved.server.key()),
                        Some(saved.id.to_string().as_str()),
                    )
                {
                    for key in ACTIVE {
                        self.settings.remove(key);
                    }
                }
            }
            Write::SetSetting(key, value) => {
                self.settings.insert(key.clone(), value.clone());
            }
            Write::RemoveSettings(keys) => {
                for key in keys {
                    self.settings.remove(key);
                }
            }
            Write::SetLastOrganization(saved, organization) => {
                if let Some(held) = self.accounts.iter_mut().find(|held| same(held, saved)) {
                    held.last_organization_id = *organization;
                }
            }
            Write::SetIdentity(saved, secret) => {
                self.identities
                    .insert((saved.server.key().to_owned(), saved.id), *secret);
            }
            Write::SetInputSettings(content) => self.input_settings = Some(content.clone()),
            Write::SetDisplaySettings(content) => self.display_settings = Some(content.clone()),
            Write::SetRemapper(content) => self.remapper.clone_from(content),
        }
    }
}

fn same(held: &SavedAccount, saved: &SavedAccount) -> bool {
    held.server.key() == saved.server.key() && held.id == saved.id
}

enum Keep {
    Here(Keeper),
    /// Handed to whatever keeps the state elsewhere: the web's host worker.
    Elsewhere(Box<dyn Fn(Write)>),
}

pub struct HostStateStore {
    held: RefCell<Held>,
    keep: Keep,
}

impl HostStateStore {
    /// Opens `host.sqlite3` here, and reads it into the copy the host reads.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, HostStateError> {
        let keeper = Keeper::open(path)?;
        Ok(Self {
            held: RefCell::new(keeper.read()?),
            keep: Keep::Here(keeper),
        })
    }

    /// The state a keeper elsewhere read (`Keeper::read`), each change to which goes to `keep`.
    pub fn kept_elsewhere(held: Held, keep: impl Fn(Write) + 'static) -> Self {
        Self {
            held: RefCell::new(held),
            keep: Keep::Elsewhere(Box::new(keep)),
        }
    }

    pub fn placeholder() -> Self {
        Self {
            held: RefCell::new(Held::default()),
            keep: Keep::Here(Keeper::in_memory()),
        }
    }

    fn write(&self, write: Write) -> Result<(), HostStateError> {
        self.held.borrow_mut().apply(&write);
        match &self.keep {
            Keep::Here(keeper) => keeper.keep(&write),
            Keep::Elsewhere(keep) => {
                keep(write);
                Ok(())
            }
        }
    }

    fn setting(&self, key: &str) -> Option<String> {
        self.held.borrow().settings.get(key).cloned()
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), HostStateError> {
        self.write(Write::SetSetting(key.to_owned(), value.to_owned()))
    }

    pub fn accounts(&self) -> Result<Vec<SavedAccount>, HostStateError> {
        Ok(self.held.borrow().accounts.clone())
    }

    pub fn save_account(&self, saved: &SavedAccount) -> Result<(), HostStateError> {
        self.write(Write::SaveAccount(saved.clone()))
    }

    pub fn remove_account(&self, saved: &SavedAccount) -> Result<(), HostStateError> {
        self.write(Write::RemoveAccount(saved.clone()))
    }

    pub fn set_active_account(&self, saved: &SavedAccount) -> Result<(), HostStateError> {
        self.set_setting(ACTIVE[0], saved.server.key())?;
        self.set_setting(ACTIVE[1], &saved.id.to_string())
    }

    pub fn clear_active_account(&self) -> Result<(), HostStateError> {
        self.write(Write::RemoveSettings(
            ACTIVE.iter().map(|key| (*key).to_owned()).collect(),
        ))
    }

    pub fn active_account(&self) -> Result<Option<(String, Uuid)>, HostStateError> {
        match (self.setting(ACTIVE[0]), self.setting(ACTIVE[1])) {
            (Some(server), Some(account)) => Ok(Some((server, parse_uuid(&account)?))),
            _ => Ok(None),
        }
    }

    pub fn set_last_organization(
        &self,
        saved: &SavedAccount,
        organization_id: Option<Uuid>,
    ) -> Result<(), HostStateError> {
        self.write(Write::SetLastOrganization(saved.clone(), organization_id))
    }

    pub fn last_organization(
        &self,
        saved: &SavedAccount,
    ) -> Result<Option<Organization>, HostStateError> {
        let Some(stored) = self.setting(&organization_key(saved)) else {
            return Ok(None);
        };
        serde_json::from_str(&stored).map(Some).map_err(|error| {
            HostStateError::from(format!("the saved organization is unreadable: {error}"))
        })
    }

    pub fn set_last_organization_details(
        &self,
        saved: &SavedAccount,
        organization: &Organization,
    ) -> Result<(), HostStateError> {
        let stored = serde_json::to_string(organization)
            .map_err(|error| HostStateError::from(error.to_string()))?;
        self.set_setting(&organization_key(saved), &stored)
    }

    pub fn warm_apps(&self) -> Result<Vec<String>, HostStateError> {
        Ok(self
            .setting("warm_apps")
            .map(|apps| {
                apps.split('\n')
                    .filter(|app| !app.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }

    pub fn set_warm_apps(&self, apps: &[String]) -> Result<(), HostStateError> {
        self.set_setting("warm_apps", &apps.join("\n"))
    }

    pub fn identity(&self, saved: &SavedAccount) -> Result<Option<[u8; 32]>, HostStateError> {
        Ok(self
            .held
            .borrow()
            .identities
            .get(&(saved.server.key().to_owned(), saved.id))
            .copied())
    }

    pub fn set_identity(
        &self,
        saved: &SavedAccount,
        secret: [u8; 32],
    ) -> Result<(), HostStateError> {
        self.write(Write::SetIdentity(saved.clone(), secret))
    }

    pub fn client_id(&self) -> Result<Uuid, HostStateError> {
        if let Some(existing) = self.setting("client_id") {
            return parse_uuid(&existing);
        }
        let id = Uuid::new_v4();
        self.set_setting("client_id", &id.to_string())?;
        Ok(id)
    }

    pub fn input_settings(&self) -> Result<Option<Vec<u8>>, HostStateError> {
        Ok(self.held.borrow().input_settings.clone())
    }

    pub fn set_input_settings(&self, content: &[u8]) -> Result<(), HostStateError> {
        self.write(Write::SetInputSettings(content.to_vec()))
    }

    pub fn remapper(&self) -> Result<Option<Vec<u8>>, HostStateError> {
        Ok(self.held.borrow().remapper.clone())
    }

    pub fn set_remapper(&self, content: Option<&[u8]>) -> Result<(), HostStateError> {
        self.write(Write::SetRemapper(content.map(<[u8]>::to_vec)))
    }

    pub fn display_settings(&self) -> Result<Option<Vec<u8>>, HostStateError> {
        Ok(self.held.borrow().display_settings.clone())
    }

    pub fn set_display_settings(&self, content: &[u8]) -> Result<(), HostStateError> {
        self.write(Write::SetDisplaySettings(content.to_vec()))
    }

    pub fn tried_display_modes(&self) -> Result<Option<String>, HostStateError> {
        Ok(self.setting("tried_display_modes"))
    }

    pub fn set_tried_display_modes(&self, modes: &str) -> Result<(), HostStateError> {
        self.set_setting("tried_display_modes", modes)
    }

    /// The keeper for what is read and written in the file itself, not copied: the push
    /// receiver's state, which another process writes too. Only where the device decrypts
    /// pushes itself (Android), which keeps its state here.
    fn here(&self) -> Result<&Keeper, HostStateError> {
        match &self.keep {
            Keep::Here(keeper) => Ok(keeper),
            Keep::Elsewhere(_) => Err(HostStateError::from(
                "this device's push receiver is kept only where it decrypts pushes itself"
                    .to_owned(),
            )),
        }
    }

    /// Keeps `keys` as this device's keys for pushes it decrypts itself (Android's UnifiedPush
    /// receiver) unless some are kept already: of two callers at once, the first one's stay.
    pub fn keep_push_keys(&self, keys: &[u8]) -> Result<(), HostStateError> {
        self.here()?.keep_push_keys(keys)
    }

    pub fn push_keys(&self) -> Result<Option<Vec<u8>>, HostStateError> {
        self.here()?.push_keys()
    }

    /// The endpoint the push distributor gave this device, until it takes it back.
    pub fn push_endpoint(&self) -> Result<Option<String>, HostStateError> {
        self.here()?.push_endpoint()
    }

    /// Keeps the endpoint with the keys it was made for: nothing is kept without keys.
    pub fn set_push_endpoint(&self, endpoint: Option<&str>) -> Result<(), HostStateError> {
        self.here()?.set_push_endpoint(endpoint)
    }

    /// Notes that the push whose sealed bytes hash to `digest` was shown at `at`, and forgets
    /// those shown before `forget_before`: false when it was shown already, so a push the server
    /// resent is dropped.
    pub fn first_showing(
        &self,
        digest: &[u8],
        at: i64,
        forget_before: i64,
    ) -> Result<bool, HostStateError> {
        self.here()?.first_showing(digest, at, forget_before)
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, HostStateError> {
    Uuid::parse_str(value).map_err(|error| HostStateError::from(error.to_string()))
}
