use std::sync::{MutexGuard, PoisonError};

use be_wasm_threads::Mutex;

#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub id: u64,
    pub message: String,
    pub report: Option<String>,
    /// A link the notice offers to open, from a press of its own (`offer_link`).
    pub link: Option<String>,
}

struct Notices {
    next: u64,
    shown: Vec<Notice>,
}

static NOTICES: Mutex<Notices> = Mutex::new(Notices {
    next: 0,
    shown: Vec::new(),
});

fn notices() -> MutexGuard<'static, Notices> {
    NOTICES.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn report(message: String) {
    show(message, None, None);
}

pub fn report_with(message: String, report: String) {
    show(message, Some(report), None);
}

/// Offers `url` for the person to open with a press, where the browser blocked a tab opened
/// without one.
pub fn offer_link(url: String) {
    show(
        format!("The browser blocked a new tab for {url}."),
        None,
        Some(url),
    );
}

fn show(message: String, report: Option<String>, link: Option<String>) {
    {
        let mut notices = notices();
        if let Some(notice) = notices
            .shown
            .iter_mut()
            .find(|notice| notice.message == message)
        {
            if report.is_some() {
                notice.report = report;
            }
            return;
        }
        notices.next += 1;
        let id = notices.next;
        notices.shown.push(Notice {
            id,
            message,
            report,
            link,
        });
    }
    crate::host::wake();
}

pub fn dismiss(id: u64) {
    notices().shown.retain(|notice| notice.id != id);
}

pub fn report_of(id: u64) -> Option<String> {
    notices()
        .shown
        .iter()
        .find(|notice| notice.id == id)
        .and_then(|notice| notice.report.clone())
}

pub fn link_of(id: u64) -> Option<String> {
    notices()
        .shown
        .iter()
        .find(|notice| notice.id == id)
        .and_then(|notice| notice.link.clone())
}

pub fn shown() -> Vec<Notice> {
    notices().shown.clone()
}

#[cfg(test)]
mod tests;
