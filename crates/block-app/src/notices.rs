use std::sync::{Mutex, MutexGuard, PoisonError};

use beui::styled::Toast;

struct Notices {
    next: u64,
    shown: Vec<Toast>,
}

static NOTICES: Mutex<Notices> = Mutex::new(Notices {
    next: 0,
    shown: Vec::new(),
});

fn notices() -> MutexGuard<'static, Notices> {
    NOTICES.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn report(message: String) {
    {
        let mut notices = notices();
        if notices.shown.iter().any(|toast| toast.message == message) {
            return;
        }
        notices.next += 1;
        let id = notices.next;
        notices.shown.push(Toast {
            id,
            message,
            danger: true,
            ..Toast::default()
        });
    }
    crate::host::wake();
}

pub(crate) fn dismiss(id: u64) {
    notices().shown.retain(|toast| toast.id != id);
}

pub(crate) fn shown() -> Vec<Toast> {
    notices().shown.clone()
}
