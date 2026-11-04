mod center;
mod history;
mod markup;
mod toasts;

use be_sdk_beui::desktop;
use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use be_sdk_beui::beui::Image;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{
    Editor, NotificationCloseReason, NotificationInbox, NotificationReport, NotificationRequest,
    NotificationsWatch, ScreenLockedWatch,
};

use center::{Center, DEFAULT_ACTION, Incoming, Notification};

pub(crate) use history::{LIST_HEIGHT, NotificationList};
pub(crate) use toasts::DesktopToasts;

pub(crate) const TOAST_IDS: u64 = 1 << 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listed {
    pub(crate) id: u32,
    pub(crate) source: String,
    pub(crate) heading: String,
    pub(crate) body: String,
    pub(crate) image: Option<Image>,
    pub(crate) critical: bool,
    pub(crate) activates: bool,
}

impl Hash for Listed {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.source.hash(state);
        self.heading.hash(state);
        self.body.hash(state);
        self.image.as_ref().map(Image::id).hash(state);
        self.critical.hash(state);
        self.activates.hash(state);
    }
}

impl Listed {
    fn of(kept: &Notification) -> Self {
        Self {
            id: kept.id,
            source: source(kept),
            heading: kept.heading().to_owned(),
            body: kept.incoming.body.clone(),
            image: kept.incoming.image.clone(),
            critical: kept.critical(),
            activates: kept.has_default_action(),
        }
    }
}

fn source(kept: &Notification) -> String {
    let at = history::received_at(kept.received);
    match kept.names_app() {
        true => format!("{} · {at}", kept.incoming.app_name),
        false => at,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DesktopNotifications {
    shared: Stored<Shared>,
    revision: ReadSignal<u64>,
    pub(crate) offered: Memo<bool>,
    locked: Memo<bool>,
    browsing: Signal<bool>,
}

struct Shared {
    editor: Editor,
    center: RefCell<Center>,
    started: Instant,
    handled: Cell<u64>,
    arrived: RefCell<NotificationInbox>,
    reported: RefCell<Vec<u32>>,
    revision: Signal<u64>,
    browsing: Signal<bool>,
    shown: Cell<u64>,
    expiry: RefCell<Option<Timer>>,
}

impl DesktopNotifications {
    pub(crate) fn new(editor: Editor) -> Self {
        let revision = create_signal(0_u64);
        let browsing = create_signal(false);
        let shared = store(Shared {
            editor,
            center: RefCell::new(Center::default()),
            started: now(),
            handled: Cell::new(0),
            arrived: RefCell::new(NotificationInbox::default()),
            reported: RefCell::new(Vec::new()),
            revision,
            browsing,
            shown: Cell::new(0),
            expiry: RefCell::new(None),
        });
        let expiry = create_timer(move || shared.try_value().and_then(|shared| shared.tick()));
        shared.with(|shared| shared.expiry.replace(Some(expiry)));
        let inbox = editor.watch::<NotificationsWatch>();
        create_effect(move || {
            let inbox = inbox.get();
            if let Some(shared) = shared.try_value() {
                untrack(|| shared.arrive(inbox));
            }
        });
        create_effect(move || {
            if browsing.get()
                && let Some(shared) = shared.try_value()
            {
                untrack(|| shared.settle());
            }
        });
        Self {
            shared,
            revision: revision.read_only(),
            offered: editor.offered::<NotificationsWatch>(),
            locked: editor.watch::<ScreenLockedWatch>(),
            browsing,
        }
    }

    pub(crate) fn listed(&self) -> Memo<Vec<Listed>> {
        let revision = self.revision;
        let shared = self.shared;
        create_memo(move || {
            revision.get();
            shared.with(|shared| shared.center.borrow().listed().map(Listed::of).collect())
        })
    }

    pub(crate) fn unread(&self) -> Memo<usize> {
        let revision = self.revision;
        let shared = self.shared;
        create_memo(move || {
            revision.get();
            shared.with(|shared| shared.center.borrow().unread())
        })
    }

    pub(crate) fn toasts(&self) -> Memo<Vec<Toast>> {
        let revision = self.revision;
        let locked = self.locked;
        let browsing = self.browsing;
        let shared = self.shared;
        create_memo(move || {
            revision.get();
            if locked.get() || browsing.get() {
                return Vec::new();
            }
            shared.with(|shared| shared.center.borrow().toasts().map(toast).collect())
        })
    }

    pub(crate) fn browsing(&self) -> Signal<bool> {
        self.browsing
    }

    pub(crate) fn dismiss(&self, ids: &[u32]) {
        let shared = self.shared.value();
        {
            let mut center = shared.center.borrow_mut();
            for id in ids {
                center.dismiss(*id);
            }
        }
        shared.settle();
    }

    pub(crate) fn invoke(&self, id: u32, action: &str) {
        let shared = self.shared.value();
        shared.center.borrow_mut().invoke(id, action);
        shared.settle();
    }

    pub(crate) fn activate(&self, id: u32) {
        self.invoke(id, DEFAULT_ACTION);
    }
}

impl Shared {
    fn elapsed(&self) -> Duration {
        now().saturating_duration_since(self.started)
    }

    fn arrive(&self, inbox: NotificationInbox) {
        self.arrived.replace(inbox);
        self.settle();
    }

    fn receive(&self, elapsed: Duration) -> Option<u64> {
        let inbox = self.arrived.take();
        let handled = self.handled.get();
        let mut received = None;
        let mut center = self.center.borrow_mut();
        for (sequence, request) in inbox.requests {
            if sequence <= handled {
                continue;
            }
            received = Some(sequence);
            match request {
                NotificationRequest::Notify(incoming) => center.notify(
                    incoming.id,
                    Incoming::from_host(&incoming),
                    elapsed,
                    incoming.received,
                ),
                NotificationRequest::Close(id) => {
                    center.close(id, NotificationCloseReason::Closed);
                }
            }
        }
        if let Some(sequence) = received {
            self.handled.set(sequence);
        }
        received
    }

    fn settle(&self) {
        let wait = self.tick();
        if let Some(expiry) = self.expiry.borrow().as_ref() {
            match wait {
                Some(wait) => expiry.restart(wait),
                None => expiry.stop(),
            }
        }
    }

    fn tick(&self) -> Option<Duration> {
        let elapsed = self.elapsed();
        let received = self.receive(elapsed);
        let (wait, signals, kept, revision) = {
            let mut center = self.center.borrow_mut();
            let wait = center.frame(elapsed);
            if self.browsing.get_untracked() {
                center.see();
            }
            (
                wait,
                center.take_signals(),
                center.kept_ids(),
                center.revision(),
            )
        };
        let moved = *self.reported.borrow() != kept;
        if received.is_some() || !signals.is_empty() || moved {
            self.reported.replace(kept.clone());
            self.editor.act(move || {
                desktop::notifications::report(NotificationReport {
                    received,
                    signals,
                    kept,
                })
                .detach()
            });
        }
        if self.shown.replace(revision) != revision {
            self.revision.set(revision);
        }
        wait
    }
}

fn toast(kept: &Notification) -> Toast {
    Toast {
        id: TOAST_IDS + u64::from(kept.id),
        source: source(kept),
        title: kept.heading().to_owned(),
        message: kept.incoming.body.clone(),
        danger: kept.critical() && kept.incoming.image.is_none(),
        image: kept.incoming.image.clone(),
        actions: kept
            .incoming
            .actions
            .iter()
            .filter(|action| action.key != DEFAULT_ACTION)
            .map(|action| ToastAction {
                key: action.key.clone(),
                label: action.label.clone(),
            })
            .collect(),
        activates: kept.has_default_action(),
        sticky: true,
    }
}

fn notification(toast: u64) -> Option<u32> {
    toast
        .checked_sub(TOAST_IDS)
        .and_then(|id| u32::try_from(id).ok())
}
