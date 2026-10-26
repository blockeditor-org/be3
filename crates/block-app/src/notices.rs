use std::sync::{MutexGuard, PoisonError};

use wasi_threads::Mutex;

use beui::styled::*;

pub(crate) const COPY_REPORT: &str = "copy-report";

struct Notice {
    toast: Toast,
    report: Option<String>,
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

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn report(message: String) {
    show(message, None);
}

pub(crate) fn report_with(message: String, report: String) {
    show(message, Some(report));
}

fn show(message: String, report: Option<String>) {
    {
        let mut notices = notices();
        let actions: Vec<ToastAction> = report
            .iter()
            .map(|_| ToastAction {
                key: COPY_REPORT.to_owned(),
                label: "Copy report".to_owned(),
            })
            .collect();
        if let Some(notice) = notices
            .shown
            .iter_mut()
            .find(|notice| notice.toast.message == message)
        {
            if report.is_some() {
                notice.report = report;
                notice.toast.actions = actions;
            }
            return;
        }
        notices.next += 1;
        let id = notices.next;
        notices.shown.push(Notice {
            toast: Toast {
                id,
                message,
                danger: true,
                actions,
                ..Toast::default()
            },
            report,
        });
    }
    crate::host::wake();
}

pub(crate) fn dismiss(id: u64) {
    notices().shown.retain(|notice| notice.toast.id != id);
}

pub(crate) fn report_of(id: u64) -> Option<String> {
    notices()
        .shown
        .iter()
        .find(|notice| notice.toast.id == id)
        .and_then(|notice| notice.report.clone())
}

pub(crate) fn shown() -> Vec<Toast> {
    notices()
        .shown
        .iter()
        .map(|notice| notice.toast.clone())
        .collect()
}

#[cfg(test)]
mod tests;
