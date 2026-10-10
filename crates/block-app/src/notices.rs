use std::cell::RefCell;

use block_plugin_api::{HostProblem, ProblemAction, Problems};

#[derive(Default)]
struct Notices {
    next: u64,
    shown: Vec<HostProblem>,
    revision: u64,
    published: Option<u64>,
}

thread_local! {
    static NOTICES: RefCell<Notices> = RefCell::new(Notices::default());
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn report(message: String) {
    NOTICES.with(|notices| {
        let mut notices = notices.borrow_mut();
        if notices.shown.iter().any(|shown| shown.message == message) {
            return;
        }
        notices.next += 1;
        notices.revision += 1;
        let id = notices.next;
        notices.shown.push(HostProblem { id, message });
    });
    crate::host::wake();
}

pub(crate) fn frame() {
    let dismissed = crate::plugin_host::take_actions::<ProblemAction>();
    let publish = NOTICES.with(|notices| {
        let mut notices = notices.borrow_mut();
        for ProblemAction::Dismiss(id) in dismissed {
            let before = notices.shown.len();
            notices.shown.retain(|shown| shown.id != id);
            if notices.shown.len() != before {
                notices.revision += 1;
            }
        }
        let revision = notices.revision;
        (notices.published != Some(revision)).then(|| {
            notices.published = Some(revision);
            notices.shown.clone()
        })
    });
    if let Some(shown) = publish {
        crate::plugin_host::publish::<Problems>(&shown);
    }
}
