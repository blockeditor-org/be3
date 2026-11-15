use std::sync::{MutexGuard, PoisonError};

use be_wasm_threads::Mutex;

#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub id: u64,
    pub message: String,
    pub report: Option<String>,
    /// A link the notice offers to open, from a press of its own (`offer_link`).
    pub link: Option<String>,
    /// Something the person asked for that was refused for a reason they can act on, not a
    /// failure (`refuse`): it reads as plain text.
    pub refusal: bool,
    /// The account notice on the server it shows (`account_notices`), which dismissing it
    /// dismisses on every device.
    pub account: Option<u64>,
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
    show(message, None, None, false);
}

pub fn report_with(message: String, report: String) {
    show(message, Some(report), None, false);
}

/// Says why something the person asked for was not done, as a plain notice.
pub fn refuse(message: String) {
    show(message, None, None, true);
}

/// Offers `url` for the person to open with a press, where the browser blocked a tab opened
/// without one.
pub fn offer_link(url: String) {
    show(
        format!("The browser blocked a new tab for {url}."),
        None,
        Some(url),
        false,
    );
}

fn show(message: String, report: Option<String>, link: Option<String>, refusal: bool) {
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
            refusal,
            account: None,
        });
    }
    crate::host::wake();
}

/// Dismisses a notice; answers the account notice it showed, which the caller dismisses on the
/// server too.
pub fn dismiss(id: u64) -> Option<u64> {
    let mut notices = notices();
    let account = notices
        .shown
        .iter()
        .find(|notice| notice.id == id)
        .and_then(|notice| notice.account);
    notices.shown.retain(|notice| notice.id != id);
    account
}

/// Shows the account's notices from the server (`account-notices`), as plain notices, and lets
/// go of those dismissed elsewhere.
pub fn account_notices(listed: Vec<(u64, String)>) {
    {
        let mut notices = notices();
        notices.shown.retain(|notice| {
            notice
                .account
                .is_none_or(|account| listed.iter().any(|(id, _)| *id == account))
        });
        for (account, message) in listed {
            if notices
                .shown
                .iter()
                .any(|notice| notice.account == Some(account))
            {
                continue;
            }
            notices.next += 1;
            let id = notices.next;
            notices.shown.push(Notice {
                id,
                message,
                report: None,
                link: None,
                refusal: true,
                account: Some(account),
            });
        }
    }
    crate::host::wake();
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
