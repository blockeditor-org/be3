//! The organization's invitations: the Invite member dialog, which makes one and ends on its
//! link, and the invite list, where an administrator sees each one's state, copies its own
//! links again, revokes outstanding ones and makes a fresh link for an expired one
//! (guides/blocks/keys.md, "Inviting").

use be_block_protocol::InvitationState;
use be_host_base::host::{Mailbox, Post};
use be_host_stack::Invite;
use uuid::Uuid;

use crate::ui::{InvitationRow, InvitationsView, InviteView};

enum Reply {
    Made(Result<String, String>),
    Listed(Result<Vec<Invite>, String>),
}

pub(crate) struct Invitations {
    /// The Invite member dialog is open.
    inviting: bool,
    /// The link it made.
    made: Option<String>,
    /// Whether an invitation is being made (`invite`), until the server answers it, whatever
    /// else the list's requests are doing.
    creating: bool,
    /// Why making the last invitation failed, apart from the list's own errors.
    make_error: Option<String>,
    /// The invite list is open.
    listing: bool,
    rows: Vec<Invite>,
    busy: bool,
    error: Option<String>,
    replies: (Mailbox, Post<Reply>),
}

impl Default for Invitations {
    fn default() -> Self {
        Self {
            inviting: false,
            made: None,
            creating: false,
            make_error: None,
            listing: false,
            rows: Vec::new(),
            busy: false,
            error: None,
            replies: crate::desk().mailbox(|core: &mut crate::HostCore, reply| {
                core.invitations.replied(reply);
            }),
        }
    }
}

impl Invitations {
    fn listed(&self) -> Box<dyn FnOnce(Result<Vec<Invite>, String>) + Send> {
        let post = self.replies.1.clone();
        Box::new(move |reply| post.send(Reply::Listed(reply)))
    }

    /// Opens the Invite member dialog and makes an invitation at once: there is nothing to
    /// fill in, since the link is for whoever it is sent to.
    pub(crate) fn invite(&mut self) {
        self.inviting = true;
        self.made = None;
        self.error = None;
        self.make_error = None;
        self.creating = true;
        self.busy = true;
        let post = self.replies.1.clone();
        be_host_stack::create_invitation(Box::new(move |made| post.send(Reply::Made(made))));
    }

    pub(crate) fn close_invite(&mut self) {
        self.inviting = false;
        self.made = None;
        self.error = None;
    }

    pub(crate) fn show(&mut self) {
        self.listing = true;
        self.error = None;
        self.busy = true;
        be_host_stack::list_invitations(self.listed());
    }

    pub(crate) fn close(&mut self) {
        let inviting = (
            self.inviting,
            self.made.take(),
            self.creating,
            self.make_error.take(),
        );
        *self = Self::default();
        (self.inviting, self.made, self.creating, self.make_error) = inviting;
    }

    pub(crate) fn revoke(&mut self, link: Uuid) {
        self.busy = true;
        self.error = None;
        be_host_stack::revoke_invitation(link, self.listed());
    }

    fn replied(&mut self, reply: Reply) {
        self.busy = false;
        if matches!(reply, Reply::Made(_)) {
            self.creating = false;
        }
        match reply {
            Reply::Made(Ok(link)) => {
                self.made = Some(link);
                if self.listing {
                    self.busy = true;
                    be_host_stack::list_invitations(self.listed());
                }
            }
            Reply::Listed(Ok(rows)) => self.rows = rows,
            Reply::Made(Err(error)) => {
                self.make_error = Some(error.clone());
                self.error = Some(error);
            }
            Reply::Listed(Err(error)) => self.error = Some(error),
        }
    }

    pub(crate) fn invite_view(&self, organization: String) -> Option<InviteView> {
        self.inviting.then(|| InviteView {
            organization,
            busy: self.busy && self.made.is_none(),
            error: self.error.clone(),
            link: self.made.clone(),
        })
    }

    pub(crate) fn view(&self) -> Option<InvitationsView> {
        self.listing.then(|| InvitationsView {
            rows: self.rows.iter().map(row).collect(),
            busy: self.busy,
            error: self.error.clone().filter(|_| !self.inviting),
        })
    }
}

fn row(invite: &Invite) -> InvitationRow {
    let open = matches!(
        invite.state,
        InvitationState::Waiting | InvitationState::Opened
    );
    let (state, detail) = match invite.state {
        InvitationState::Waiting => (
            "Waiting".to_owned(),
            format!("Not opened yet · expires {}", date(invite.expires)),
        ),
        InvitationState::Opened => (
            "Opened".to_owned(),
            format!(
                "Opened {} · expires {}",
                date(invite.opened),
                date(invite.expires)
            ),
        ),
        InvitationState::Accepted => (
            "Accepted".to_owned(),
            format!("By {} on {}", invite.accepter, date(invite.ended)),
        ),
        InvitationState::Expired => ("Expired".to_owned(), format!("On {}", date(invite.expires))),
        InvitationState::Revoked => ("Revoked".to_owned(), format!("On {}", date(invite.ended))),
    };
    InvitationRow {
        id: invite.id,
        title: format!(
            "{state} · made by {} on {}",
            invite.inviter,
            date(invite.created)
        ),
        detail,
        link: invite.link.clone().filter(|_| open),
        revocable: open,
        renewable: invite.state == InvitationState::Expired,
    }
}

/// What a root guest is shown and handed (`be3:root`'s `core-view` and `make-secret`;
/// root/block_stack.rs).
impl Invitations {
    /// The organization's invitations as a root shows them: listed once `show` asked and the
    /// server answered.
    pub(crate) fn root_area(&self) -> be_wasm_root_api::InvitationsArea {
        use be_wasm_root_api::InvitationState as State;
        let at = |milliseconds: i64| (milliseconds != 0).then_some(milliseconds);
        be_wasm_root_api::InvitationsArea {
            listed: self.listing && !self.busy,
            rows: self
                .rows
                .iter()
                .map(|invite| be_wasm_root_api::InvitationRow {
                    id: invite.id.into_bytes(),
                    state: match invite.state {
                        InvitationState::Waiting => State::Waiting,
                        InvitationState::Opened => State::Opened,
                        InvitationState::Accepted => State::Accepted,
                        InvitationState::Expired => State::Expired,
                        InvitationState::Revoked => State::Revoked,
                    },
                    inviter: invite.inviter.clone(),
                    accepter: (!invite.accepter.is_empty()).then(|| invite.accepter.clone()),
                    created: invite.created,
                    opened: at(invite.opened),
                    expires: invite.expires,
                    ended: at(invite.ended),
                    has_link: open_link(invite).is_some(),
                })
                .collect(),
            busy: self.busy,
            error: self.error.clone().filter(|_| !self.inviting),
        }
    }

    /// The link of the invitation `id` again, while it is one of the person's own and still
    /// open.
    pub(crate) fn link_of(&self, id: Uuid) -> Option<String> {
        self.rows
            .iter()
            .find(|invite| invite.id == id)
            .and_then(open_link)
    }

    /// How the invitation `invite` asked for stands: its link once made, or why it failed;
    /// none while it is being made.
    pub(crate) fn made(&self) -> Option<Result<String, String>> {
        if let Some(link) = &self.made {
            return Some(Ok(link.clone()));
        }
        match (self.creating, &self.make_error) {
            (true, _) => None,
            (false, Some(error)) => Some(Err(error.clone())),
            (false, None) => Some(Err("no invitation was made".to_owned())),
        }
    }
}

/// An invitation's link, for one of the person's own still open.
fn open_link(invite: &Invite) -> Option<String> {
    matches!(
        invite.state,
        InvitationState::Waiting | InvitationState::Opened
    )
    .then(|| invite.link.clone())
    .flatten()
}

/// A day in UTC, from milliseconds since the epoch, as `2026-11-15`.
fn date(milliseconds: i64) -> String {
    let days = milliseconds.div_euclid(86_400_000);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}-{day:02}")
}
