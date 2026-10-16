use uuid::Uuid;

use super::{AppStateError, SavedAccount};

const ACCOUNTS_KEY: &str = "block.accounts";
const ACTIVE_KEY: &str = "block.active-account";
const CLIENT_ID_KEY: &str = "block.client-id";
const DISPLAY_SETTINGS_KEY: &str = "block.display-settings";
const INPUT_SETTINGS_KEY: &str = "block.input-settings";
const REMAPPER_KEY: &str = "block.remapper";
const IDENTITY_KEY: &str = "block.identity-secrets";

#[derive(serde::Deserialize, serde::Serialize)]
struct StoredIdentity {
    server: String,
    account: Uuid,
    secret: [u8; 32],
}

pub struct AppStateStore {
    storage: web_sys::Storage,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ActiveAccount {
    server: String,
    id: Uuid,
}

impl AppStateStore {
    pub fn open() -> Result<Self, AppStateError> {
        let storage = web_sys::window()
            .ok_or_else(|| AppStateError::from("no browser window is available".to_owned()))?
            .local_storage()
            .map_err(|_| AppStateError::from("browser storage is unavailable".to_owned()))?
            .ok_or_else(|| {
                AppStateError::from(
                    "browser storage is disabled, so accounts cannot be saved".to_owned(),
                )
            })?;
        Ok(Self { storage })
    }

    pub fn accounts(&self) -> Result<Vec<SavedAccount>, AppStateError> {
        let mut accounts: Vec<SavedAccount> = self.read(ACCOUNTS_KEY)?.unwrap_or_default();

        accounts.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(accounts)
    }

    pub fn save_account(&self, saved: &SavedAccount) -> Result<(), AppStateError> {
        let mut accounts = self.accounts()?;
        match accounts
            .iter_mut()
            .find(|account| account.server == saved.server && account.id == saved.id)
        {
            Some(existing) => *existing = saved.clone(),
            None => accounts.push(saved.clone()),
        }
        self.write(ACCOUNTS_KEY, &accounts)
    }

    pub fn remove_account(&self, saved: &SavedAccount) -> Result<(), AppStateError> {
        let mut accounts = self.accounts()?;
        accounts.retain(|account| account.server != saved.server || account.id != saved.id);
        self.write(ACCOUNTS_KEY, &accounts)?;
        let mut identities = self.stored_identities()?;
        identities
            .retain(|stored| stored.server != saved.server.key() || stored.account != saved.id);
        self.write(IDENTITY_KEY, &identities)?;
        if self.active_account()?.as_ref() == Some(&(saved.server.key().into(), saved.id)) {
            self.clear_active_account()?;
        }
        Ok(())
    }

    pub fn set_active_account(&self, saved: &SavedAccount) -> Result<(), AppStateError> {
        self.write(
            ACTIVE_KEY,
            &ActiveAccount {
                server: saved.server.key().to_owned(),
                id: saved.id,
            },
        )
    }

    pub fn clear_active_account(&self) -> Result<(), AppStateError> {
        self.storage
            .remove_item(ACTIVE_KEY)
            .map_err(|_| AppStateError::from("failed to clear the active account".to_owned()))
    }

    pub fn clear(&self) -> Result<(), AppStateError> {
        for key in [
            ACCOUNTS_KEY,
            ACTIVE_KEY,
            CLIENT_ID_KEY,
            DISPLAY_SETTINGS_KEY,
            INPUT_SETTINGS_KEY,
            REMAPPER_KEY,
            IDENTITY_KEY,
        ] {
            self.storage
                .remove_item(key)
                .map_err(|_| AppStateError::from(format!("failed to clear {key}")))?;
        }
        Ok(())
    }

    pub fn active_account(&self) -> Result<Option<(String, Uuid)>, AppStateError> {
        let active: Option<ActiveAccount> = self.read(ACTIVE_KEY)?;
        Ok(active.map(|active| (active.server, active.id)))
    }

    pub fn set_last_organization(
        &self,
        saved: &SavedAccount,
        organization_id: Option<Uuid>,
    ) -> Result<(), AppStateError> {
        let mut accounts = self.accounts()?;
        let Some(account) = accounts
            .iter_mut()
            .find(|account| account.server == saved.server && account.id == saved.id)
        else {
            return Ok(());
        };
        account.last_organization_id = organization_id;
        self.write(ACCOUNTS_KEY, &accounts)
    }

    fn stored_identities(&self) -> Result<Vec<StoredIdentity>, AppStateError> {
        Ok(self.read(IDENTITY_KEY)?.unwrap_or_default())
    }

    pub fn identity(&self, saved: &SavedAccount) -> Result<Option<[u8; 32]>, AppStateError> {
        Ok(self
            .stored_identities()?
            .into_iter()
            .find(|stored| stored.server == saved.server.key() && stored.account == saved.id)
            .map(|stored| stored.secret))
    }

    pub fn set_identity(
        &self,
        saved: &SavedAccount,
        secret: [u8; 32],
    ) -> Result<(), AppStateError> {
        let mut identities = self.stored_identities()?;
        identities
            .retain(|stored| stored.server != saved.server.key() || stored.account != saved.id);
        identities.push(StoredIdentity {
            server: saved.server.key().to_owned(),
            account: saved.id,
            secret,
        });
        self.write(IDENTITY_KEY, &identities)
    }

    pub fn client_id(&self) -> Result<Uuid, AppStateError> {
        if let Some(id) = self.read(CLIENT_ID_KEY)? {
            return Ok(id);
        }
        let id = Uuid::new_v4();
        self.write(CLIENT_ID_KEY, &id)?;
        Ok(id)
    }

    pub fn input_settings(&self) -> Result<Option<Vec<u8>>, AppStateError> {
        self.read(INPUT_SETTINGS_KEY)
    }

    pub fn set_input_settings(&self, content: &[u8]) -> Result<(), AppStateError> {
        self.write(INPUT_SETTINGS_KEY, &content)
    }

    pub fn remapper(&self) -> Result<Option<Vec<u8>>, AppStateError> {
        self.read(REMAPPER_KEY)
    }

    pub fn set_remapper(&self, content: Option<&[u8]>) -> Result<(), AppStateError> {
        self.write(REMAPPER_KEY, &content)
    }

    pub fn display_settings(&self) -> Result<Option<Vec<u8>>, AppStateError> {
        self.read(DISPLAY_SETTINGS_KEY)
    }

    pub fn set_display_settings(&self, content: &[u8]) -> Result<(), AppStateError> {
        self.write(DISPLAY_SETTINGS_KEY, &content)
    }

    fn read<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>, AppStateError> {
        let Some(text) = self
            .storage
            .get_item(key)
            .map_err(|_| AppStateError::from(format!("failed to read {key}")))?
        else {
            return Ok(None);
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| AppStateError::from(format!("stored {key} is unreadable: {error}")))
    }

    fn write<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<(), AppStateError> {
        let text = serde_json::to_string(value)
            .map_err(|error| AppStateError::from(format!("failed to encode {key}: {error}")))?;
        self.storage.set_item(key, &text).map_err(|_| {
            AppStateError::from(format!(
                "failed to save {key}, which usually means browser storage is full"
            ))
        })
    }
}
