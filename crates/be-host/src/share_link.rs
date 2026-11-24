//! Opening a share link someone sent (guides/blocks/keys.md, "Share links"): pasted, or from the
//! web build's address bar. The person is told what the link shares, in which organization, and
//! whether opening it makes them a guest there, before anything is opened.

use be_block_keys::InviteCode;
use be_host_base::host::{Mailbox, Post};
use be_host_stack::SharePreview;
use be_host_stack::invite_link::InviteLink;
use uuid::Uuid;

use crate::ui::ShareLinkView;

enum Reply {
    Previewed(InviteCode, Result<Option<SharePreview>, String>),
    /// The share link an invitation carries, read once its contact link made the contact.
    Invited(InviteCode, Result<Option<SharePreview>, String>),
    Opened(Result<SharePreview, String>),
}

pub(crate) struct ShareLinks {
    open: bool,
    offered: Option<(InviteCode, SharePreview)>,
    opened: Option<SharePreview>,
    /// The organization a link just opened is in, until the host looks whether it is the one
    /// open (`take_elsewhere`).
    landed: Option<Uuid>,
    /// A link from the address bar, opened once the account is ready.
    waiting: Option<String>,
    busy: bool,
    error: Option<String>,
    replies: (Mailbox, Post<Reply>),
}

impl Default for ShareLinks {
    fn default() -> Self {
        Self {
            open: false,
            offered: None,
            opened: None,
            landed: None,
            waiting: None,
            busy: false,
            error: None,
            replies: crate::desk().mailbox(|core: &mut crate::HostCore, reply| {
                core.share_links.replied(reply);
            }),
        }
    }
}

impl ShareLinks {
    pub(crate) fn show(&mut self) {
        self.open = true;
        self.offered = None;
        self.opened = None;
        self.error = None;
    }

    /// Closes the dialog, keeping a link from the address bar not yet opened, and the mailbox.
    pub(crate) fn close(&mut self) {
        self.open = false;
        self.offered = None;
        self.opened = None;
        self.landed = None;
        self.busy = false;
        self.error = None;
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn open_from_address(&mut self, text: &str) {
        self.waiting = Some(text.to_owned());
    }

    pub(crate) fn poll(&mut self, base: &str, ready: bool) {
        if ready && let Some(text) = self.waiting.take() {
            self.paste(&text, base);
        }
    }

    pub(crate) fn paste(&mut self, text: &str, base: &str) {
        self.open = true;
        self.opened = None;
        let Some(link) = InviteLink::parse_share(text) else {
            self.error = Some(
                "That is not a share link. Copy the whole link from the person who sent it."
                    .to_owned(),
            );
            return;
        };
        if !link.is_for(base) {
            self.error = Some(format!(
                "This share link is for {}. Switch to an account on that server to open it.",
                link.server.unwrap_or_default()
            ));
            return;
        }
        let post = self.replies.1.clone();
        let code = link.code.clone();
        self.busy = true;
        self.error = None;
        be_host_stack::preview_share_link(link.code, move |previewed| {
            post.send(Reply::Previewed(code, previewed));
        });
    }

    /// Reads the share link an invitation carries with its contact link's code
    /// (plans/invitations_without_pending_grants.md), once the contact is made, and offers it as
    /// a pasted one: a contact link that carries none shows nothing.
    pub(crate) fn follow_invitation(&mut self, code: InviteCode) {
        let post = self.replies.1.clone();
        let carried = code.clone();
        be_host_stack::preview_share_link(code, move |previewed| {
            post.send(Reply::Invited(carried, previewed));
        });
    }

    pub(crate) fn accept(&mut self) {
        let Some((code, shown)) = self.offered.take() else {
            return;
        };
        let post = self.replies.1.clone();
        self.busy = true;
        be_host_stack::open_share_link(code, (shown.block, shown.access), move |opened| {
            post.send(Reply::Opened(opened))
        });
    }

    pub(crate) fn decline(&mut self) {
        self.offered = None;
    }

    /// The organization a link just opened is in, when it is not the one open: the host opens
    /// that one, where the block is shared with this account.
    pub(crate) fn take_elsewhere(&mut self, current: Option<Uuid>) -> Option<Uuid> {
        self.landed.take().filter(|landed| Some(*landed) != current)
    }

    fn replied(&mut self, reply: Reply) {
        self.busy = false;
        match reply {
            Reply::Previewed(code, Ok(Some(preview))) => self.offered = Some((code, preview)),
            Reply::Previewed(_, Ok(None)) => {
                self.error = Some("That link does not exist, or was revoked.".to_owned());
            }
            Reply::Invited(code, Ok(Some(preview))) => {
                self.open = true;
                self.opened = None;
                self.error = None;
                self.offered = Some((code, preview));
            }
            Reply::Invited(_, Ok(None)) => {}
            Reply::Invited(_, Err(error)) => {
                self.open = true;
                self.error = Some(error);
            }
            Reply::Opened(Ok(opened)) => {
                self.landed = Some(opened.organization);
                self.opened = Some(opened);
            }
            Reply::Previewed(_, Err(error)) | Reply::Opened(Err(error)) => self.error = Some(error),
        }
    }

    pub(crate) fn view(&self) -> Option<ShareLinkView> {
        if !self.open {
            return None;
        }
        Some(ShareLinkView {
            offer: self.offered.as_ref().map(|(_, preview)| offered(preview)),
            opened: self.opened.as_ref().map(|opened| {
                format!(
                    "{} is open to you: you find it under Shared with me in {}.",
                    named(opened),
                    organization(opened)
                )
            }),
            busy: self.busy,
            error: self.error.clone(),
        })
    }
}

/// The most of a block's name the dialog shows: the name is its maker's word, sealed with the
/// link's code, so it is shown as text on one line and cut short.
const MAX_NAME_CHARS: usize = 60;

fn named(preview: &SharePreview) -> String {
    let words: Vec<&str> = preview
        .name
        .split(|character: char| character.is_whitespace() || character.is_control())
        .filter(|word| !word.is_empty())
        .collect();
    let name: String = words
        .join(" ")
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    match name.chars().count() {
        0 => "The block".to_owned(),
        count if count > MAX_NAME_CHARS => format!(
            "“{}…”",
            name.chars().take(MAX_NAME_CHARS).collect::<String>()
        ),
        _ => format!("“{name}”"),
    }
}

fn organization(preview: &SharePreview) -> String {
    match preview.organization_name.trim().is_empty() {
        true => "its organization".to_owned(),
        false => preview.organization_name.clone(),
    }
}

/// What the person is asked before opening a link: what it shares, where, what it lets them
/// do, and whether they join the organization as a guest.
fn offered(preview: &SharePreview) -> String {
    let can = match preview.access {
        be_block_graph::Access::Edit => "view and edit it",
        be_block_graph::Access::Join => "join it while someone hosts it",
        _ => "view it",
    };
    let guest = match preview.guest {
        true => format!(
            " You will join {} as a guest: you see only what links share with you there.",
            organization(preview)
        ),
        false => String::new(),
    };
    format!(
        "Open {} in {}? You can {can}.{guest}",
        named(preview),
        organization(preview)
    )
}
