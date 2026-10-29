use be_keys::{RecoveryPhrase, RecoverySecret};
use uuid::Uuid;

use crate::accounts::{self, AccountError, Keys};
use block_app_base::platform;

#[derive(Default)]
pub struct KeyState {
    keys: Option<Keys>,
    setup: Option<RecoverySetup>,
    saving: Option<platform::RequestResult<Result<(), AccountError>>>,
    pairing: Option<PairingWait>,
    error: Option<String>,
}

struct RecoverySetup {
    phrase: RecoveryPhrase,
    checked: [usize; 3],
    replacing: bool,
}

struct PairingWait {
    code: String,
    receiver: platform::RequestResult<Result<[u8; 32], AccountError>>,
    _cancel: tokio::sync::oneshot::Sender<()>,
}

pub enum KeyEvent {
    RecoverySaved([u8; 32]),
    Unlocked([u8; 32]),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecoveryView {
    pub words: Vec<String>,
    pub checked: Vec<usize>,
    pub busy: bool,
    pub replacing: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnlockView {
    pub account: String,
    pub code: Option<String>,
    pub error: Option<String>,
}

impl KeyState {
    pub fn loaded(&mut self, keys: Keys) {
        if keys.recovery.is_none() && self.setup.is_none() {
            let phrase = RecoveryPhrase::generate();
            let checked = phrase.checked_positions();
            self.setup = Some(RecoverySetup {
                phrase,
                checked,
                replacing: false,
            });
        }
        if keys.recovery.is_some() && self.setup.as_ref().is_some_and(|setup| !setup.replacing) {
            self.setup = None;
        }
        self.keys = Some(keys);
    }

    pub fn needs_recovery(&self) -> bool {
        self.keys
            .as_ref()
            .is_some_and(|keys| keys.recovery.is_none())
    }

    pub fn needs_unlock(&self, identity: Option<[u8; 32]>) -> bool {
        let Some(recovery) = self.keys.as_ref().and_then(|keys| keys.recovery) else {
            return false;
        };
        identity.is_none_or(|secret| RecoverySecret::from_bytes(secret).public() != recovery)
    }

    pub fn replacing(&self) -> bool {
        self.setup.as_ref().is_some_and(|setup| setup.replacing)
    }

    pub fn replace_recovery(&mut self) {
        let phrase = RecoveryPhrase::generate();
        let checked = phrase.checked_positions();
        self.error = None;
        self.setup = Some(RecoverySetup {
            phrase,
            checked,
            replacing: true,
        });
    }

    pub fn cancel_replacing(&mut self) {
        if self.replacing() && self.saving.is_none() {
            self.setup = None;
            self.error = None;
        }
    }

    pub fn confirm_recovery(
        &mut self,
        typed: &[String],
        server_url: String,
        token: String,
        identity: Option<[u8; 32]>,
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
        self.save_recovery(server_url, token, identity);
    }

    pub fn skip_confirmation(
        &mut self,
        server_url: String,
        token: String,
        identity: Option<[u8; 32]>,
    ) {
        self.save_recovery(server_url, token, identity);
    }

    fn save_recovery(&mut self, server_url: String, token: String, identity: Option<[u8; 32]>) {
        let Some(setup) = &self.setup else {
            return;
        };
        if self.saving.is_some() {
            return;
        }
        let fresh = setup.phrase.secret();
        let public = fresh.public();
        let (account, held, own) = self
            .keys
            .as_ref()
            .map(|keys| (keys.account, keys.sealed.clone(), keys.own.clone()))
            .unwrap_or_default();
        let resealed = match (held.is_empty() && own.is_empty(), identity) {
            (true, _) => Some(be_client::contacts::NewIdentity {
                sealed: Vec::new(),
                own: Vec::new(),
                changes: Vec::new(),
            }),
            (false, None) => None,
            (false, Some(identity)) => be_client::contacts::new_identity(
                account,
                &RecoverySecret::from_bytes(identity),
                &fresh,
                held,
                own,
            ),
        };
        let Some(resealed) = resealed else {
            self.error = Some(
                "This device cannot open what the old phrase sealed. Unlock this account with \
                 the old phrase first, so the new one can seal it."
                    .into(),
            );
            return;
        };
        self.error = None;
        self.saving = Some(platform::spawn_request(accounts::set_recovery_key(
            server_url, token, public, resealed,
        )));
    }

    pub fn unlock_with_phrase(&mut self, typed: &str) -> Option<[u8; 32]> {
        let opened = (|| {
            let phrase = RecoveryPhrase::parse(typed).map_err(|error| error.to_string())?;
            let keys = self
                .keys
                .as_ref()
                .ok_or("This account's keys have not loaded yet.")?;
            let secret = phrase.secret();
            if keys.recovery != Some(secret.public()) {
                return Err("That is not this account's recovery phrase.".to_owned());
            }
            Ok(secret.to_bytes())
        })();
        match opened {
            Ok(opened) => {
                self.error = None;
                Some(opened)
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }

    pub fn start_pairing(&mut self, server_url: String, token: String, account: Uuid) {
        let code = be_keys::pairing_code();
        let (cancel, cancelled) = tokio::sync::oneshot::channel::<()>();
        let receiver = platform::spawn_request(accounts::pair(
            server_url,
            token,
            account,
            code.clone(),
            std::env::consts::OS.to_owned(),
            Box::pin(async move {
                let _ = cancelled.await;
            }),
        ));
        self.error = None;
        self.pairing = Some(PairingWait {
            code,
            receiver,
            _cancel: cancel,
        });
    }

    pub fn cancel_pairing(&mut self) {
        self.pairing = None;
    }

    pub fn poll(&mut self) -> Option<KeyEvent> {
        if let Some(saving) = &self.saving
            && let Ok(result) = saving.try_recv()
        {
            self.saving = None;
            match result {
                Ok(()) => {
                    self.keys = None;
                    if let Some(setup) = self.setup.take() {
                        return Some(KeyEvent::RecoverySaved(setup.phrase.secret().to_bytes()));
                    }
                }
                Err(error) => self.error = Some(error.message),
            }
        }
        if let Some(pairing) = &self.pairing
            && let Ok(result) = pairing.receiver.try_recv()
        {
            self.pairing = None;
            match result {
                Ok(secret) => {
                    let matches = self.keys.as_ref().and_then(|keys| keys.recovery)
                        == Some(RecoverySecret::from_bytes(secret).public());
                    if matches {
                        return Some(KeyEvent::Unlocked(secret));
                    }
                    self.error = Some(
                        "The other device handed over a key that is not this account's. Try \
                         again from a device that is unlocked."
                            .into(),
                    );
                }
                Err(error) => self.error = Some(error.message),
            }
        }
        None
    }

    pub fn recovery_view(&self) -> RecoveryView {
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
            replacing: self.replacing(),
            error: self.error.clone(),
        }
    }

    pub fn unlock_view(&self, account: &str) -> UnlockView {
        UnlockView {
            account: account.to_owned(),
            code: self
                .pairing
                .as_ref()
                .map(|pairing| format!("{}-{}", &pairing.code[..4], &pairing.code[4..])),
            error: self.error.clone(),
        }
    }
}
