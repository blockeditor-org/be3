use std::cell::Cell;

use be_block_keys::{RecoveryPhrase, RecoveryPublic, RecoverySecret};
use uuid::Uuid;

use crate::accounts::{self, AccountError, Keys};
use be_host_base::host::{Mailbox, Post};
use be_host_base::platform;

pub type Answer<T> = (Mailbox, Post<Result<T, AccountError>>);

#[derive(Default)]
pub struct KeyState {
    keys: Option<Keys>,
    setup: Option<RecoverySetup>,
    saving: Option<Mailbox>,
    pairing: Option<PairingWait>,
    error: Option<String>,
    derived: Cell<Option<([u8; 32], RecoveryPublic)>>,
    /// The server says the account has no recovery key, though this device holds its secret.
    lost: bool,
}

struct RecoverySetup {
    phrase: RecoveryPhrase,
    checked: [usize; 3],
    replacing: bool,
}

struct PairingWait {
    code: String,
    _answer: Mailbox,
    _cancel: tokio::sync::oneshot::Sender<()>,
}

pub enum KeyEvent {
    RecoverySaved([u8; 32]),
    Unlocked([u8; 32]),
}

#[derive(Clone, Default, PartialEq)]
pub struct RecoveryView {
    pub words: Vec<String>,
    pub checked: Vec<usize>,
    pub busy: bool,
    pub replacing: bool,
    pub error: Option<String>,
}

/// Leaves out the phrase's words, so no log ever holds them.
impl std::fmt::Debug for RecoveryView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoveryView")
            .field("words", &self.words.len())
            .field("checked", &self.checked)
            .field("busy", &self.busy)
            .field("replacing", &self.replacing)
            .field("error", &self.error)
            .finish()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnlockView {
    pub account: String,
    pub code: Option<String>,
    pub error: Option<String>,
}

/// Why a device that holds an account's identity secret makes it no new phrase when the server
/// says the account has none (guides/blocks/keys.md, "Identity, new phrases and devices").
pub const SERVER_LOST_KEY: &str = "The server says this account has no recovery key, but this \
     device holds one. Nothing was replaced: your phrase still works. Ask the server's operator \
     to restore the account's data.";

impl KeyState {
    /// Takes the account's keys as the server answers them. `held` is the identity secret this
    /// device keeps for the account: with one, a server saying the account has no key is never
    /// taken as a reason to make a new phrase, which would replace the secret the device keeps
    /// (a server could otherwise have every device of the account drop its key). Pass none for a
    /// local account, whose server is this device.
    pub fn loaded(&mut self, keys: Keys, held: Option<[u8; 32]>) {
        self.lost = keys.recovery.is_none() && held.is_some();
        if self.lost {
            self.setup = None;
            self.error = Some(SERVER_LOST_KEY.into());
        }
        if keys.recovery.is_none() && self.setup.is_none() && !self.lost {
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
        !self.lost
            && self
                .keys
                .as_ref()
                .is_some_and(|keys| keys.recovery.is_none())
    }

    /// Why the account's keys can't be used here: the server says it has none, while this device
    /// holds one (`loaded`).
    pub fn lost(&self) -> Option<&'static str> {
        self.lost.then_some(SERVER_LOST_KEY)
    }

    pub fn needs_unlock(&self, identity: Option<[u8; 32]>) -> bool {
        let Some(recovery) = self.keys.as_ref().and_then(|keys| keys.recovery) else {
            return false;
        };
        identity.is_none_or(|secret| self.public_of(secret) != recovery)
    }

    fn public_of(&self, secret: [u8; 32]) -> RecoveryPublic {
        match self.derived.get() {
            Some((held, public)) if held == secret => public,
            _ => {
                let public = RecoverySecret::from_bytes(secret).public();
                self.derived.set(Some((secret, public)));
                public
            }
        }
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
        saved: Answer<()>,
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
        self.save_recovery(server_url, token, identity, saved);
    }

    pub fn skip_confirmation(
        &mut self,
        server_url: String,
        token: String,
        identity: Option<[u8; 32]>,
        saved: Answer<()>,
    ) {
        self.save_recovery(server_url, token, identity, saved);
    }

    fn save_recovery(
        &mut self,
        server_url: String,
        token: String,
        identity: Option<[u8; 32]>,
        (mailbox, post): Answer<()>,
    ) {
        let Some(setup) = &self.setup else {
            return;
        };
        if self.saving.is_some() {
            return;
        }
        let fresh = setup.phrase.secret();
        let (account, holds) = self
            .keys
            .as_ref()
            .map(|keys| {
                (
                    keys.account,
                    !(keys.sealed.is_empty() && keys.logs.is_empty()),
                )
            })
            .unwrap_or_default();
        // Told at once when it shows: the reseal itself runs where it is saved, after the
        // contacts' waiting key changes are followed (`accounts::set_recovery_key`).
        if holds && identity.is_none() {
            self.error = Some(accounts::CANNOT_RESEAL.into());
            return;
        }
        self.error = None;
        let saving = accounts::set_recovery_key(
            server_url,
            token,
            account,
            identity.map(RecoverySecret::from_bytes),
            fresh,
            crate::contact_pins(),
        );
        platform::spawn_request(async move { post.send(saving.await) });
        self.saving = Some(mailbox);
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

    pub fn start_pairing(
        &mut self,
        server_url: String,
        token: String,
        account: Uuid,
        (mailbox, post): Answer<[u8; 32]>,
    ) {
        let code = be_block_keys::pairing_code();
        let (cancel, cancelled) = tokio::sync::oneshot::channel::<()>();
        let pairing = accounts::pair(
            server_url,
            token,
            account,
            code.clone(),
            std::env::consts::OS.to_owned(),
            Box::pin(async move {
                let _ = cancelled.await;
            }),
        );
        platform::spawn_request(async move { post.send(pairing.await) });
        self.error = None;
        self.pairing = Some(PairingWait {
            code,
            _answer: mailbox,
            _cancel: cancel,
        });
    }

    pub fn cancel_pairing(&mut self) {
        self.pairing = None;
    }

    pub fn saved(&mut self, result: Result<(), AccountError>) -> Option<KeyEvent> {
        self.saving = None;
        match result {
            Ok(()) => {
                self.keys = None;
                let setup = self.setup.take()?;
                Some(KeyEvent::RecoverySaved(setup.phrase.secret().to_bytes()))
            }
            Err(error) => {
                self.error = Some(error.message);
                None
            }
        }
    }

    pub fn paired(&mut self, result: Result<[u8; 32], AccountError>) -> Option<KeyEvent> {
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
                None
            }
            Err(error) => {
                self.error = Some(error.message);
                None
            }
        }
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
