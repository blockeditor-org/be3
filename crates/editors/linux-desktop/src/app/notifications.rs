mod center;
mod history;
mod markup;
mod toasts;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use block_editor_beui::beui::reactive::{
    Memo, ReadSignal, Timer, WriteSignal, clone, create_effect, create_memo, create_signal,
    create_timer, now, untrack,
};
use block_editor_beui::beui::styled::{Toast, ToastAction};
use block_editor_beui::{
    Editor, NotificationCloseReason, NotificationInbox, NotificationReport, NotificationRequest,
    Notifications, ScreenLocked,
};

use center::{Center, DEFAULT_ACTION, Incoming, Notification};

pub(crate) use history::NotificationsButton;
pub(crate) use toasts::DesktopToasts;

pub(crate) const TOAST_IDS: u64 = 1 << 32;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Listed {
    pub(crate) id: u32,
    pub(crate) app_name: String,
    pub(crate) summary: String,
    pub(crate) body: String,
    pub(crate) received: u64,
    pub(crate) critical: bool,
    pub(crate) activates: bool,
}

impl Listed {
    fn of(kept: &Notification) -> Self {
        Self {
            id: kept.id,
            app_name: kept.incoming.app_name.clone(),
            summary: kept.incoming.summary.clone(),
            body: kept.incoming.body.clone(),
            received: kept.received,
            critical: kept.critical(),
            activates: kept.has_default_action(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct DesktopNotifications {
    shared: Rc<Shared>,
    revision: ReadSignal<u64>,
    locked: Memo<bool>,
    browsing: ReadSignal<bool>,
    set_browsing: WriteSignal<bool>,
}

struct Shared {
    editor: Editor,
    center: RefCell<Center>,
    started: Instant,
    handled: Cell<u64>,
    arrived: RefCell<NotificationInbox>,
    reported: RefCell<Vec<u32>>,
    revision: WriteSignal<u64>,
    shown: Cell<u64>,
    expiry: RefCell<Option<Timer>>,
}

impl DesktopNotifications {
    pub(crate) fn new(editor: &Editor) -> Self {
        let (revision, set_revision) = create_signal(0_u64);
        let shared = Rc::new(Shared {
            editor: editor.clone(),
            center: RefCell::new(Center::default()),
            started: now(),
            handled: Cell::new(0),
            arrived: RefCell::new(NotificationInbox::default()),
            reported: RefCell::new(Vec::new()),
            revision: set_revision,
            shown: Cell::new(0),
            expiry: RefCell::new(None),
        });
        let weak = Rc::downgrade(&shared);
        let expiry = create_timer(clone!(weak -> move || {
            weak.upgrade().and_then(|shared| shared.tick())
        }));
        shared.expiry.replace(Some(expiry));
        let inbox = editor.host_value::<Notifications>();
        create_effect(move || {
            let inbox = inbox.get();
            if let Some(shared) = weak.upgrade() {
                untrack(|| shared.arrive(inbox));
            }
        });
        let (browsing, set_browsing) = create_signal(false);
        Self {
            shared,
            revision,
            locked: editor.host_value::<ScreenLocked>(),
            browsing,
            set_browsing,
        }
    }

    pub(crate) fn listed(&self) -> Memo<Vec<Listed>> {
        let revision = self.revision.clone();
        let shared = Rc::clone(&self.shared);
        create_memo(move || {
            revision.get();
            shared.center.borrow().listed().map(Listed::of).collect()
        })
    }

    pub(crate) fn toasts(&self) -> Memo<Vec<Toast>> {
        let revision = self.revision.clone();
        let locked = self.locked.clone();
        let browsing = self.browsing.clone();
        let shared = Rc::clone(&self.shared);
        create_memo(move || {
            revision.get();
            if locked.get() || browsing.get() {
                return Vec::new();
            }
            shared.center.borrow().toasts().map(toast).collect()
        })
    }

    pub(crate) fn browse(&self, open: bool) {
        self.set_browsing.set(open);
    }

    pub(crate) fn dismiss(&self, ids: &[u32]) {
        {
            let mut center = self.shared.center.borrow_mut();
            for id in ids {
                center.dismiss(*id);
            }
        }
        self.shared.settle();
    }

    pub(crate) fn invoke(&self, id: u32, action: &str) {
        self.shared.center.borrow_mut().invoke(id, action);
        self.shared.settle();
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
            self.editor.act(NotificationReport {
                received,
                signals,
                kept,
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
        title: kept.incoming.summary.clone(),
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
