use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::version_control_data::CommitId;
use block_client::version_control_checkout::{
    CheckoutOutcome, checkout_worktree, worktree_is_clean,
};
use block_client::version_control_commit::{CommitOutcome, commit_worktree};
use block_editor_plugin::beui::reactive::{
    Memo, ReadSignal, WriteSignal, create_memo, create_signal,
};
use block_editor_plugin::{Editor, EditorHost, Task};
use uuid::Uuid;

use super::unix_seconds_now;

const COMMIT_RACE: &str = "The branch moved before the commit landed; try again.";
const COMMIT_FAILED: &str = "Commit failed.";
const CHECKOUT_FAILED: &str = "Checkout failed.";

pub(crate) struct Work {
    host: EditorHost,
    client: Arc<BlockClient>,
    worktree: Uuid,
    dirty: ReadSignal<Option<bool>>,
    set_dirty: WriteSignal<Option<bool>>,
    checking: ReadSignal<bool>,
    set_checking: WriteSignal<bool>,
    committing: ReadSignal<bool>,
    set_committing: WriteSignal<bool>,
    switching: ReadSignal<bool>,
    set_switching: WriteSignal<bool>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    awaiting: ReadSignal<Option<CommitId>>,
    set_awaiting: WriteSignal<Option<CommitId>>,
    status: RefCell<Option<Task<Option<bool>>>>,
    commit: RefCell<Option<Task<Option<CommitOutcome>>>>,
    checkout: RefCell<Option<Task<Option<CheckoutOutcome>>>>,
    target: RefCell<Option<CommitId>>,
}

impl Work {
    pub(crate) fn new(editor: &Editor) -> Rc<Self> {
        let (dirty, set_dirty) = create_signal(None::<bool>);
        let (checking, set_checking) = create_signal(false);
        let (committing, set_committing) = create_signal(false);
        let (switching, set_switching) = create_signal(false);
        let (error, set_error) = create_signal(None::<String>);
        let (awaiting, set_awaiting) = create_signal(None::<CommitId>);
        let work = Rc::new(Self {
            host: editor.host().clone(),
            client: Arc::clone(editor.client()),
            worktree: editor.block_id(),
            dirty,
            set_dirty,
            checking,
            set_checking,
            committing,
            set_committing,
            switching,
            set_switching,
            error,
            set_error,
            awaiting,
            set_awaiting,
            status: RefCell::new(None),
            commit: RefCell::new(None),
            checkout: RefCell::new(None),
            target: RefCell::new(None),
        });
        let polled = Rc::clone(&work);
        editor.each_frame(move || polled.poll());
        work
    }

    pub(crate) fn dirty(&self) -> Memo<Option<bool>> {
        let dirty = self.dirty.clone();
        create_memo(move || dirty.get())
    }

    pub(crate) fn checking(&self) -> Memo<bool> {
        let checking = self.checking.clone();
        create_memo(move || checking.get())
    }

    pub(crate) fn committing(&self) -> Memo<bool> {
        let committing = self.committing.clone();
        create_memo(move || committing.get())
    }

    pub(crate) fn switching(&self) -> Memo<bool> {
        let switching = self.switching.clone();
        create_memo(move || switching.get())
    }

    pub(crate) fn error(&self) -> Memo<Option<String>> {
        let error = self.error.clone();
        create_memo(move || error.get())
    }

    pub(crate) fn awaiting(&self) -> Memo<Option<CommitId>> {
        let awaiting = self.awaiting.clone();
        create_memo(move || awaiting.get())
    }

    pub(crate) fn dismiss(&self) {
        self.set_awaiting.set(None);
    }

    pub(crate) fn refresh(&self) {
        if self.status.borrow().is_some() {
            return;
        }
        let client = Arc::clone(&self.client);
        let worktree = self.worktree;
        *self.status.borrow_mut() = Some(
            self.host
                .spawn(async move { worktree_is_clean(&client, worktree).await }),
        );
        self.set_checking.set(true);
    }

    pub(crate) fn commit(&self, author: Uuid, message: String) {
        let message = message.trim().to_owned();
        if self.commit.borrow().is_some() || message.is_empty() {
            return;
        }
        let client = Arc::clone(&self.client);
        let worktree = self.worktree;
        let time = unix_seconds_now();
        *self.commit.borrow_mut() =
            Some(self.host.spawn(async move {
                commit_worktree(&client, worktree, author, time, message).await
            }));
        self.set_committing.set(true);
    }

    pub(crate) fn switch(&self, target: CommitId, discard: bool) {
        if self.checkout.borrow().is_some() {
            return;
        }
        let client = Arc::clone(&self.client);
        let worktree = self.worktree;
        let wanted = target.clone();
        *self.target.borrow_mut() = Some(target);
        *self.checkout.borrow_mut() = Some(
            self.host
                .spawn(async move { checkout_worktree(&client, worktree, wanted, discard).await }),
        );
        self.set_switching.set(true);
    }

    fn poll(&self) {
        if let Some(clean) = finish(&self.status) {
            self.set_checking.set(false);
            self.set_dirty.set(clean.flatten().map(|clean| !clean));
        }
        self.poll_commit();
        self.poll_checkout();
        if self.dirty.get_untracked().is_none() && self.status.borrow().is_none() {
            self.refresh();
        }
    }

    fn poll_commit(&self) {
        let Some(outcome) = finish(&self.commit) else {
            return;
        };
        self.set_committing.set(false);
        match outcome.flatten() {
            Some(outcome) if outcome.branch_advanced => self.set_error.set(None),
            Some(_) => self.set_error.set(Some(COMMIT_RACE.to_owned())),
            None => self.set_error.set(Some(COMMIT_FAILED.to_owned())),
        }
        self.set_dirty.set(None);
    }

    fn poll_checkout(&self) {
        let Some(outcome) = finish(&self.checkout) else {
            return;
        };
        self.set_switching.set(false);
        let target = self.target.borrow_mut().take();
        match outcome.flatten() {
            Some(CheckoutOutcome::Applied { .. }) => {
                self.set_awaiting.set(None);
                self.set_error.set(None);
                self.set_dirty.set(None);
            }
            Some(CheckoutOutcome::Blocked) => self.set_awaiting.set(target),
            None => self.set_error.set(Some(CHECKOUT_FAILED.to_owned())),
        }
    }
}

fn finish<T>(slot: &RefCell<Option<Task<T>>>) -> Option<Option<T>> {
    let mut slot = slot.borrow_mut();
    let task = slot.as_mut()?;
    let result = task.take();
    if !task.finished() {
        return None;
    }
    *slot = None;
    Some(result)
}
