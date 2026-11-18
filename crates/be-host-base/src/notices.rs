use std::sync::{MutexGuard, PoisonError};

use std::sync::Mutex;

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

/// The key of the action a notice with a report offers: copying the report.
pub const COPY_REPORT: &str = "copy-report";

/// The key of the action a notice with a link offers: opening the link, which dismisses it.
pub const OPEN_LINK: &str = "open-link";

impl Notice {
    /// Whether it says something failed, which reads as an alert and stays until dismissed,
    /// rather than offering a link or saying why something was refused.
    pub fn failure(&self) -> bool {
        self.link.is_none() && !self.refusal
    }

    /// What it offers the person to do beside dismissing it: each action's key
    /// (`COPY_REPORT`, `OPEN_LINK`) and label.
    pub fn actions(&self) -> Vec<(&'static str, &'static str)> {
        let report = self.report.iter().map(|_| (COPY_REPORT, "Copy report"));
        let link = self.link.iter().map(|_| (OPEN_LINK, "Open"));
        report.chain(link).collect()
    }
}

struct Notices {
    next: u64,
    shown: Vec<Notice>,
    /// Account notices dismissed on this device, which stay hidden here though the server
    /// keeps them for the account's other devices.
    dismissed: Vec<u64>,
}

static NOTICES: Mutex<Notices> = Mutex::new(Notices {
    next: 0,
    shown: Vec::new(),
    dismissed: Vec::new(),
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

/// Dismisses a notice; answers the account notice it showed, which stays hidden on this device
/// and which the caller dismisses on the server too (where it changes nothing in its first week).
pub fn dismiss(id: u64) -> Option<u64> {
    let mut notices = notices();
    let account = notices
        .shown
        .iter()
        .find(|notice| notice.id == id)
        .and_then(|notice| notice.account);
    notices.shown.retain(|notice| notice.id != id);
    if let Some(account) = account {
        notices.dismissed.push(account);
    }
    account
}

/// Shows the account's notices from the server (`query.notices`), as plain notices, and lets
/// go of those dismissed elsewhere.
pub fn account_notices(listed: Vec<(u64, String)>) {
    {
        let mut notices = notices();
        notices.shown.retain(|notice| {
            notice
                .account
                .is_none_or(|account| listed.iter().any(|(id, _)| *id == account))
        });
        notices
            .dismissed
            .retain(|dismissed| listed.iter().any(|(id, _)| id == dismissed));
        for (account, message) in listed {
            if notices.dismissed.contains(&account)
                || notices
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
