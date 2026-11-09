//! What the editor knows of boat.dev: the viewer's key, the sandboxes it last read, and the
//! reading itself, which polls while the editor is on screen.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::Duration;

use be_sdk_beui::be_block::{SettingsLayer, ViewState};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::{ContentProjection, Editor, KeptEdit, LayeredSettings, Task};
use be_type_boat::{Boat, BoatContent, BoatKey, BoatSettings, BoatTerminal, TerminalAgent};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::terminals::{self, TerminalStatus};
use crate::api::{self, Account, Action, BoatError, Phase, Sandbox};
use crate::time::{self, DAY};

/// The viewer's view state key under which the last list is kept.
pub(crate) const CACHE: &str = "boat.sandboxes";
/// How often the list is read while the editor is on screen.
const POLL: Duration = Duration::from_secs(15);
/// How often while a sandbox starts or stops.
const POLL_MOVING: Duration = Duration::from_secs(3);
/// How often the clock the list's age and the key's expiry are told by moves on.
const CLOCK: Duration = Duration::from_secs(60);
/// How old the kept list may grow before an unchanged reading is kept again, so its age
/// stays close to the truth without writing the view block on every poll.
const KEEP_AGAIN: i64 = 5 * 60;
/// How long before the key's expiry the editor starts warning.
const WARN_BEFORE: i64 = 14 * DAY;

/// The sandboxes as read at `fetched`, for the account and filters they were read with.
/// The viewer's view state keeps the last of these, so the editor opens with it at once.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Listed {
    pub fetched: i64,
    pub filters: Filters,
    pub sandboxes: Vec<Sandbox>,
    /// More sandboxes are listed than the editor reads (`api::PAGES` pages).
    pub cut_off: bool,
}

/// What the block asks to see.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Filters {
    pub organization: String,
    pub name_prefix: String,
    pub label: String,
}

impl Filters {
    fn of(boat: &Boat) -> Self {
        Self {
            organization: boat.organization.clone(),
            name_prefix: boat.name_prefix.clone(),
            label: boat.label.clone(),
        }
    }

    fn label_filter(&self) -> Option<(&str, &str)> {
        Boat::label_filter(&self.label)
    }
}

/// How the key stands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum KeyStanding {
    Good,
    /// Expires within two weeks, at this time.
    Expiring(i64),
    /// Past the expiry the person entered, or boat said so.
    Expired,
    /// boat no longer takes it.
    Refused,
    /// It lacks the actions a terminal needs (`exec`, `file.write`).
    Lacking,
}

#[derive(Clone, Copy)]
pub(crate) struct State {
    pub editor: Editor,
    pub content: ContentProjection<BoatContent>,
    pub filters: Memo<Filters>,
    pub key: Memo<Option<BoatKey>>,
    /// boat said it no longer takes the key, so nothing is asked until it is replaced.
    refused: Memo<bool>,
    /// Shown instead of the editor: the host is a web page, which boat's CORS refuses.
    pub web: Memo<bool>,
    /// The sign-in page is open over a key the person wants to replace.
    pub reconnecting: Signal<bool>,
    /// The list shown: the last reading, or the one the view state kept.
    pub listed: Memo<Option<Listed>>,
    /// Why the last reading failed, while it is the latest word.
    pub failure: Signal<Option<BoatError>>,
    /// Starts and stops asked for that the list doesn't show moving yet.
    pub asked: Signal<BTreeMap<String, Action>>,
    /// Why the last start or stop was refused.
    pub notice: Signal<Option<String>>,
    /// The wall clock as of the last reading, for the list's age.
    pub now: Signal<i64>,
    /// The viewer's own terminals, from their owner-only settings, by terminal block.
    pub own_terminals: Memo<BTreeMap<Uuid, TerminalAgent>>,
    /// The terminals the block references, by terminal block id.
    pub content_terminals: ReadSignal<BTreeMap<String, BoatTerminal>>,
    /// The referenced terminals the viewer can open.
    pub visible_terminals: Memo<BTreeSet<Uuid>>,
    /// How the viewer's own terminals stand in this editor.
    pub terminal_status: Signal<BTreeMap<Uuid, TerminalStatus>>,
    /// Terminals whose agent is being made, by sandbox.
    pub connecting: Signal<BTreeMap<String, Uuid>>,
    /// boat refused a terminal's command for the key's actions: it needs a new key.
    pub lacking: Signal<bool>,
    /// Whether the viewer may not edit the block, read here once since an answer's callback
    /// runs outside any scope.
    pub read_only: Memo<bool>,
    pub(super) fetched: Signal<Option<Listed>>,
    reading: Stored<RefCell<Option<Task>>>,
    pub(super) settings: LayeredSettings<BoatSettings>,
    polling: StoredCell<Option<Timer>>,
}

impl State {
    pub(crate) fn new(editor: Editor) -> Self {
        let content = editor.block_content::<BoatContent>();
        let filters = {
            let read = content.project(|content| Filters::of(&content.root()));
            create_memo(move || read.get())
        };
        // Waits for the viewer's settings, so a key already kept shows the list at once
        // rather than the sign-in page first.
        let settings = LayeredSettings::<BoatSettings>::this_device(editor);
        let held = settings.effective();
        let key = create_memo(move || {
            let organization = filters.with(|filters| filters.organization.clone());
            held.with(|held| held.key(&organization).cloned())
        });
        let on_the_web = !editor.host().page_origin().is_empty();
        let web = create_memo(move || on_the_web);
        let kept = create_memo(move || {
            editor
                .view_state(CACHE)
                .and_then(|state| state.value::<Listed>())
        });
        let fetched = create_signal(None::<Listed>);
        let listed = create_memo(move || {
            let filters = filters.get();
            fetched
                .get()
                .or_else(|| kept.get())
                .filter(|listed| listed.filters == filters)
        });
        let failure = create_signal(None::<BoatError>);
        let refused = create_memo(move || {
            failure.with(|failure| matches!(failure, Some(BoatError::Reconnect { .. })))
        });
        let state = Self {
            editor,
            content,
            filters,
            key,
            refused,
            web,
            reconnecting: create_signal(false),
            listed,
            failure,
            asked: create_signal(BTreeMap::new()),
            notice: create_signal(None),
            now: create_signal(time::now()),
            own_terminals: terminals::own_terminals(held),
            content_terminals: content.project(|content| {
                content
                    .root()
                    .terminals
                    .iter()
                    .map(|(block, terminal)| (block.clone(), terminal.clone()))
                    .collect()
            }),
            visible_terminals: terminals::visible_terminals(editor),
            terminal_status: create_signal(BTreeMap::new()),
            connecting: create_signal(BTreeMap::new()),
            lacking: create_signal(false),
            read_only: editor.read_only(),
            fetched,
            reading: store(RefCell::new(None)),
            settings,
            polling: StoredCell::new(None),
        };
        let polling = create_timer(move || {
            state.read(false);
            state.visible().then(|| state.interval())
        });
        state.polling.set(Some(polling));
        create_effect(move || {
            if state.account().is_some() {
                untrack(|| state.read(true));
            }
        });
        let connected = create_memo(move || state.account().is_some());
        let shown = editor.presence_visible();
        create_effect(move || match shown.get() && connected.get() {
            true => polling.start(untrack(|| state.interval())),
            false => polling.stop(),
        });
        // The list's age and the key's expiry move on between readings too.
        let now = state.now;
        let clock = create_timer(move || {
            now.set(time::now());
            state.visible().then_some(CLOCK)
        });
        create_effect(move || match shown.get() {
            true => clock.start(CLOCK),
            false => clock.stop(),
        });
        state.watch_terminals();
        let reading = state.reading;
        on_cleanup(move || {
            if let Some(task) = reading
                .try_with(|reading| reading.borrow_mut().take())
                .flatten()
            {
                task.cancel();
            }
        });
        state
    }

    fn visible(&self) -> bool {
        self.editor.presence_visible().get_untracked()
    }

    fn interval(&self) -> Duration {
        let moving = self.asked.with_untracked(|asked| !asked.is_empty())
            || self.listed.with_untracked(|listed| {
                listed.as_ref().is_some_and(|listed| {
                    listed
                        .sandboxes
                        .iter()
                        .any(|sandbox| sandbox.phase().moving())
                })
            });
        match moving {
            true => POLL_MOVING,
            false => POLL,
        }
    }

    /// The key and account to call boat with, while the key is one boat may still take.
    pub(crate) fn account(&self) -> Option<Account> {
        if self.web.get() {
            return None;
        }
        let key = self.key.get()?;
        let expired = key.expires > 0 && key.expires <= time::now();
        (!self.refused.get() && !expired).then(|| Account {
            key: key.secret,
            organization: self.filters.with(|filters| filters.organization.clone()),
        })
    }

    pub(crate) fn standing(&self) -> Option<KeyStanding> {
        let key = self.key.get()?;
        let now = self.now.get();
        Some(match self.failure.get() {
            Some(BoatError::Reconnect { expired: true }) => KeyStanding::Expired,
            Some(BoatError::Reconnect { expired: false }) => KeyStanding::Refused,
            _ if self.lacking.get() => KeyStanding::Lacking,
            _ if key.expires > 0 && key.expires <= now => KeyStanding::Expired,
            _ if key.expires > 0 && key.expires - now <= WARN_BEFORE => {
                KeyStanding::Expiring(key.expires)
            }
            _ => KeyStanding::Good,
        })
    }

    /// Reads the list; `again` replaces a reading still on its way (the filters changed),
    /// and otherwise one on its way is left to finish.
    pub(crate) fn read(&self, again: bool) {
        let Some(account) = untrack(|| self.account()) else {
            return;
        };
        let running = self
            .reading
            .with(|reading| reading.borrow().clone())
            .filter(|task| !task.is_finished());
        if let Some(task) = running {
            if !again {
                return;
            }
            task.cancel();
        }
        let filters = self.filters.get_untracked();
        let state = *self;
        let asked = api::sandboxes(
            &self.editor.host(),
            &account,
            &filters.name_prefix,
            filters.label_filter(),
        );
        let task = self.editor.then(asked, move |answer| {
            let now = time::now();
            state.now.set(now);
            match answer {
                Ok((sandboxes, cut_off)) => state.listed_now(Listed {
                    fetched: now,
                    filters,
                    sandboxes,
                    cut_off,
                }),
                Err(error) => state.failure.set(Some(error)),
            }
        });
        self.reading.with(|reading| reading.replace(Some(task)));
    }

    fn listed_now(&self, listed: Listed) {
        self.failure.set(None);
        self.asked.update(|asked| {
            asked.retain(|id, action| {
                listed
                    .sandboxes
                    .iter()
                    .find(|sandbox| sandbox.id == *id)
                    .is_some_and(|sandbox| {
                        matches!(
                            (action, sandbox.phase()),
                            (Action::Resume, Phase::Stopped) | (Action::Stop, Phase::Running)
                        )
                    })
            });
        });
        let kept = self
            .editor
            .view_state(CACHE)
            .and_then(|state| state.value::<Listed>());
        let keep = kept.is_none_or(|kept| {
            kept.filters != listed.filters
                || kept.sandboxes != listed.sandboxes
                || listed.fetched - kept.fetched >= KEEP_AGAIN
        });
        if keep {
            self.editor
                .set_view_state(CACHE, Some(&ViewState::new(&listed, Vec::new())));
        }
        self.fetched.set(Some(listed));
    }

    /// Where a sandbox shows as being, counting a start or stop asked for and not yet seen.
    pub(crate) fn phase(&self, sandbox: &Sandbox) -> Phase {
        let phase = sandbox.phase();
        match (
            self.asked.with(|asked| asked.get(&sandbox.id).copied()),
            phase,
        ) {
            (Some(Action::Resume), Phase::Stopped) => Phase::Starting,
            (Some(Action::Stop), Phase::Running) => Phase::Stopping,
            _ => phase,
        }
    }

    /// Starts or stops a sandbox, then reads the list sooner to follow it.
    pub(crate) fn act(&self, sandbox: String, action: Action) {
        let Some(account) = untrack(|| self.account()) else {
            return;
        };
        self.notice.set(None);
        self.asked.update(|asked| {
            asked.insert(sandbox.clone(), action);
        });
        let state = *self;
        let asked = api::act(&self.editor.host(), &account, &sandbox, action);
        self.editor.then(asked, move |answer| match answer {
            Ok(()) => {
                if let Some(polling) = state.polling.get() {
                    polling.restart(POLL_MOVING);
                }
            }
            Err(error) => {
                state.asked.update(|asked| {
                    asked.remove(&sandbox);
                });
                match error {
                    BoatError::Reconnect { .. } => state.failure.set(Some(error)),
                    error => state.notice.set(Some(error.to_string())),
                }
            }
        });
    }

    pub(crate) fn refresh(&self) {
        self.read(true);
    }

    /// Keeps a key boat accepted for the block's account, in the viewer's own owner-only
    /// settings; false when the settings it goes in haven't loaded yet.
    #[must_use]
    pub(crate) fn keep_key(&self, key: BoatKey) -> bool {
        let organization = self.filters.get_untracked().organization;
        let edit: KeptEdit<BoatSettings> =
            Rc::new(move |_| BoatSettings::set_key(&organization, Some(&key)));
        if !self.settings.write_all(SettingsLayer::EveryDevice, &[edit]) {
            return false;
        }
        self.failure.set(None);
        self.lacking.set(false);
        self.reconnecting.set(false);
        self.now.set(time::now());
        true
    }

    pub(crate) fn set_organization(&self, organization: String) {
        self.content.operate(Boat::set_organization(organization));
    }

    pub(crate) fn set_name_prefix(&self, prefix: String) {
        self.content.operate(Boat::set_name_prefix(prefix));
    }

    pub(crate) fn set_label(&self, label: String) {
        self.content.operate(Boat::set_label(label));
    }
}
