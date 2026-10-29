use std::sync::{MutexGuard, PoisonError};

use wasi_threads::Mutex;

#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub id: u64,
    pub message: String,
    pub report: Option<String>,
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
    show(message, None);
}

pub fn report_with(message: String, report: String) {
    show(message, Some(report));
}

fn show(message: String, report: Option<String>) {
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

pub fn shown() -> Vec<Notice> {
    notices().shown.clone()
}

#[cfg(test)]
mod tests;
