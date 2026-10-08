use std::cell::RefCell;

use beui::styled::Toast;

#[derive(Default)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct Notices {
    next: u64,
    shown: Vec<Toast>,
}

thread_local! {
    static NOTICES: RefCell<Notices> = RefCell::new(Notices::default());
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn report(message: String) {
    NOTICES.with(|notices| {
        let mut notices = notices.borrow_mut();
        if notices.shown.iter().any(|toast| toast.message == message) {
            return;
        }
        notices.next += 1;
        let id = notices.next;
        notices.shown.push(Toast {
            id,
            message,
            danger: true,
        });
    });
    crate::host::wake();
}

pub(crate) fn dismiss(id: u64) {
    NOTICES.with(|notices| notices.borrow_mut().shown.retain(|toast| toast.id != id));
}

pub(crate) fn shown() -> Vec<Toast> {
    NOTICES.with(|notices| notices.borrow().shown.clone())
}
