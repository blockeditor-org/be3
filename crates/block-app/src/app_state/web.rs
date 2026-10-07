use uuid::Uuid;

use super::{AppStateError, SavedAccount};

const ACCOUNTS_KEY: &str = "block.accounts";
const ACTIVE_KEY: &str = "block.active-account";
const CLIENT_ID_KEY: &str = "block.client-id";
const INPUT_SETTINGS_KEY: &str = "block.input-settings";
const WORKSPACE_KEYS_KEY: &str = "block.workspace-keys";

#[derive(serde::Deserialize, serde::Serialize)]
struct StoredKey {
    server: String,
    account: Uuid,
    workspace: Uuid,
    key: [u8; 32],
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
        let mut keys = self.stored_keys()?;
        keys.retain(|stored| stored.server != saved.server.key() || stored.account != saved.id);
        self.write(WORKSPACE_KEYS_KEY, &keys)?;
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
        for key in [ACCOUNTS_KEY, ACTIVE_KEY, CLIENT_ID_KEY, WORKSPACE_KEYS_KEY] {
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

    pub fn set_last_workspace(
        &self,
        saved: &SavedAccount,
        workspace_id: Option<Uuid>,
    ) -> Result<(), AppStateError> {
        let mut accounts = self.accounts()?;
        let Some(account) = accounts
            .iter_mut()
            .find(|account| account.server == saved.server && account.id == saved.id)
        else {
            return Ok(());
        };
        account.last_workspace_id = workspace_id;
        self.write(ACCOUNTS_KEY, &accounts)
    }

    fn stored_keys(&self) -> Result<Vec<StoredKey>, AppStateError> {
        Ok(self.read(WORKSPACE_KEYS_KEY)?.unwrap_or_default())
    }

    pub fn workspace_key(
        &self,
        saved: &SavedAccount,
        workspace: Uuid,
    ) -> Result<Option<[u8; 32]>, AppStateError> {
        Ok(self
            .workspace_keys(saved)?
            .into_iter()
            .find(|(stored, _)| *stored == workspace)
            .map(|(_, key)| key))
    }

    pub fn workspace_keys(
        &self,
        saved: &SavedAccount,
    ) -> Result<Vec<(Uuid, [u8; 32])>, AppStateError> {
        Ok(self
            .stored_keys()?
            .into_iter()
            .filter(|stored| stored.server == saved.server.key() && stored.account == saved.id)
            .map(|stored| (stored.workspace, stored.key))
            .collect())
    }

    pub fn set_workspace_key(
        &self,
        saved: &SavedAccount,
        workspace: Uuid,
        key: [u8; 32],
    ) -> Result<(), AppStateError> {
        let mut keys = self.stored_keys()?;
        keys.retain(|stored| {
            stored.server != saved.server.key()
                || stored.account != saved.id
                || stored.workspace != workspace
        });
        keys.push(StoredKey {
            server: saved.server.key().to_owned(),
            account: saved.id,
            workspace,
            key,
        });
        self.write(WORKSPACE_KEYS_KEY, &keys)
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
