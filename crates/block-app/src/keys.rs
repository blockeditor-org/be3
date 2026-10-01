use be_keys::RecoveryPhrase;
use uuid::Uuid;

use crate::accounts::{self, AccountError, Keys};
use crate::platform;

#[derive(Default)]
pub(crate) struct KeyState {
    keys: Option<Keys>,
    setup: Option<RecoverySetup>,
    saving: Option<platform::RequestResult<Result<(), AccountError>>>,
    pairing: Option<PairingWait>,
    error: Option<String>,
}

struct RecoverySetup {
    phrase: RecoveryPhrase,
    checked: [usize; 3],
}

struct PairingWait {
    workspace: Uuid,
    code: String,
    receiver: platform::RequestResult<Result<[u8; 32], AccountError>>,
    _cancel: tokio::sync::oneshot::Sender<()>,
}

pub(crate) enum KeyEvent {
    RecoverySaved,
    Unlocked(Uuid, [u8; 32]),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RecoveryView {
    pub(crate) words: Vec<String>,
    pub(crate) checked: Vec<usize>,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct UnlockView {
    pub(crate) workspace: String,
    pub(crate) sealed: bool,
    pub(crate) code: Option<String>,
    pub(crate) error: Option<String>,
}

impl KeyState {
    pub(crate) fn loaded(&mut self, keys: Keys) {
        if keys.recovery.is_none() && self.setup.is_none() {
            let phrase = RecoveryPhrase::generate();
            let checked = phrase.checked_positions();
            self.setup = Some(RecoverySetup { phrase, checked });
        }
        if keys.recovery.is_some() {
            self.setup = None;
        }
        self.keys = Some(keys);
    }

    pub(crate) fn needs_recovery(&self) -> bool {
        self.keys
            .as_ref()
            .is_some_and(|keys| keys.recovery.is_none())
    }

    pub(crate) fn confirm_recovery(
        &mut self,
        typed: &[String],
        server_url: String,
        token: String,
        held: Vec<(Uuid, [u8; 32])>,
    ) {
        let Some(setup) = &self.setup else {
            return;
        };
        let words = setup.phrase.words();
        let matches = typed.len() == setup.checked.len()
            && setup
                .checked
                .iter()
                .zip(typed)
                .all(|(position, word)| words[*position].eq_ignore_ascii_case(word.trim()));
        if !matches {
            self.error =
                Some("Those words do not match the phrase. Check them and try again.".into());
            return;
        }
        self.save_recovery(server_url, token, held);
    }

    pub(crate) fn skip_confirmation(
        &mut self,
        server_url: String,
        token: String,
        held: Vec<(Uuid, [u8; 32])>,
    ) {
        self.save_recovery(server_url, token, held);
    }

    fn save_recovery(&mut self, server_url: String, token: String, held: Vec<(Uuid, [u8; 32])>) {
        let Some(setup) = &self.setup else {
            return;
        };
        if self.saving.is_some() {
            return;
        }
        let public = setup.phrase.secret().public();
        let sealed = held
            .into_iter()
            .map(|(workspace, key)| be_protocol::SealedKey {
                workspace,
                sealed: public.seal(&key),
            })
            .collect();
        self.error = None;
        self.saving = Some(platform::spawn_request(accounts::set_recovery_key(
            server_url, token, public, sealed,
        )));
    }

    pub(crate) fn unlock_with_phrase(&mut self, workspace: Uuid, typed: &str) -> Option<[u8; 32]> {
        let opened = (|| {
            let phrase = RecoveryPhrase::parse(typed).map_err(|error| error.to_string())?;
            let keys = self
                .keys
                .as_ref()
                .ok_or("The workspace keys have not loaded yet.")?;
            let secret = phrase.secret();
            if keys.recovery != Some(secret.public()) {
                return Err("That is not this account's recovery phrase.".to_owned());
            }
            let sealed = keys
                .sealed
                .get(&workspace)
                .ok_or("No key for this workspace is sealed to your recovery phrase yet.")?;
            let key = secret.open(sealed).map_err(|error| error.to_string())?;
            <[u8; 32]>::try_from(key).map_err(|_| "The sealed key is corrupt.".to_owned())
        })();
        match opened {
            Ok(key) => {
                self.error = None;
                Some(key)
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }

    pub(crate) fn start_pairing(&mut self, server_url: String, token: String, workspace: Uuid) {
        let code = be_keys::pairing_code();
        let (cancel, cancelled) = tokio::sync::oneshot::channel::<()>();
        let receiver = platform::spawn_request(accounts::pair(
            server_url,
            token,
            workspace,
            code.clone(),
            std::env::consts::OS.to_owned(),
            Box::pin(async move {
                let _ = cancelled.await;
            }),
        ));
        self.error = None;
        self.pairing = Some(PairingWait {
            workspace,
            code,
            receiver,
            _cancel: cancel,
        });
    }

    pub(crate) fn cancel_pairing(&mut self) {
        self.pairing = None;
    }

    pub(crate) fn poll(&mut self) -> Option<KeyEvent> {
        if let Some(saving) = &self.saving
            && let Ok(result) = saving.try_recv()
        {
            self.saving = None;
            match result {
                Ok(()) => {
                    self.setup = None;
                    self.keys = None;
                    return Some(KeyEvent::RecoverySaved);
                }
                Err(error) => self.error = Some(error.message),
            }
        }
        if let Some(pairing) = &self.pairing
            && let Ok(result) = pairing.receiver.try_recv()
        {
            let workspace = pairing.workspace;
            self.pairing = None;
            match result {
                Ok(key) => return Some(KeyEvent::Unlocked(workspace, key)),
                Err(error) => self.error = Some(error.message),
            }
        }
        None
    }

    pub(crate) fn recovery_view(&self) -> RecoveryView {
        RecoveryView {
            words: self
                .setup
                .as_ref()
                .map(|setup| {
                    setup
                        .phrase
                        .words()
                        .into_iter()
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            checked: self
                .setup
                .as_ref()
                .map(|setup| setup.checked.to_vec())
                .unwrap_or_default(),
            busy: self.saving.is_some(),
            error: self.error.clone(),
        }
    }

    pub(crate) fn unlock_view(&self, workspace: Uuid, name: &str) -> UnlockView {
        UnlockView {
            workspace: name.to_owned(),
            sealed: self
                .keys
                .as_ref()
                .is_some_and(|keys| keys.sealed.contains_key(&workspace)),
            code: self
                .pairing
                .as_ref()
                .filter(|pairing| pairing.workspace == workspace)
                .map(|pairing| format!("{}-{}", &pairing.code[..4], &pairing.code[4..])),
            error: self.error.clone(),
        }
    }
}
