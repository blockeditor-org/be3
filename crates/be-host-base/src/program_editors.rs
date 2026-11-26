//! The editors of the programs the desktop lists, which the host hands its apps and its root
//! with the registry's block types. A change is pushed to everyone hearing them ([`Hearing`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use be_sdk_api::EditorDescriptor;

struct Listed {
    editors: Vec<EditorDescriptor>,
    hearings: Vec<Weak<AtomicBool>>,
}

static LISTED: Mutex<Listed> = Mutex::new(Listed {
    editors: Vec::new(),
    hearings: Vec::new(),
});

fn held() -> MutexGuard<'static, Listed> {
    LISTED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Hears the program editors listed: they are due to it when it is made, and again each time
/// they change (`set`) or it asks for them `again`.
pub struct Hearing(Arc<AtomicBool>);

impl Default for Hearing {
    fn default() -> Self {
        let due = Arc::new(AtomicBool::new(true));
        let mut held = held();
        held.hearings.retain(|hearing| hearing.strong_count() > 0);
        held.hearings.push(Arc::downgrade(&due));
        Self(due)
    }
}

impl Hearing {
    /// The editors listed, if they are due: taking them leaves them due no longer.
    pub fn take(&self) -> Option<Vec<EditorDescriptor>> {
        self.0.swap(false, Ordering::AcqRel).then(listed)
    }

    /// Makes the editors due again, for a hearer that lost what it was handed.
    pub fn again(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// The editors listed now.
pub fn listed() -> Vec<EditorDescriptor> {
    held().editors.clone()
}

/// Lists `editors`, making them due to every hearing when they changed.
pub fn set(editors: Vec<EditorDescriptor>) {
    let mut held = held();
    if held.editors == editors {
        return;
    }
    held.editors = editors;
    held.hearings.retain(|hearing| match hearing.upgrade() {
        Some(due) => {
            due.store(true, Ordering::Release);
            true
        }
        None => false,
    });
    drop(held);
    crate::host::wake();
}
