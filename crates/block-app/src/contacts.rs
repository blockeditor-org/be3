use std::sync::mpsc::{Receiver, Sender, channel};

use be_keys::{ContactCard, InviteCode};
use uuid::Uuid;

use crate::invite_link::InviteLink;
use crate::ui::{ContactRow, ContactsView};

enum Reply {
    Link(Result<String, String>),
    Opened(InviteCode, Result<ContactCard, String>),
    Accepted(Result<ContactCard, String>),
}

struct Offered {
    code: InviteCode,
    card: ContactCard,
}

pub(crate) struct Contacts {
    open: bool,
    link: Option<String>,
    offered: Option<Offered>,
    removing: Option<Uuid>,
    waiting: Option<String>,
    busy: bool,
    error: Option<String>,
    connected: Option<String>,
    replies: (Sender<Reply>, Receiver<Reply>),
    published: Option<(Option<(Uuid, Uuid)>, u64)>,
}

impl Default for Contacts {
    fn default() -> Self {
        Self {
            open: false,
            link: None,
            offered: None,
            removing: None,
            waiting: None,
            busy: false,
            error: None,
            connected: None,
            replies: channel(),
            published: None,
        }
    }
}

impl Contacts {
    pub(crate) fn show(&mut self) {
        self.open = true;
        self.link = None;
        self.error = None;
        self.connected = None;
    }

    pub(crate) fn close(&mut self) {
        *self = Self {
            waiting: self.waiting.take(),
            ..Self::default()
        };
    }

    pub(crate) fn new_link(&mut self) {
        let sender = self.replies.0.clone();
        self.busy = true;
        self.error = None;
        self.link = None;
        crate::be::make_contact_link(None, move |made| {
            let _ = sender.send(Reply::Link(made));
            crate::host::wake();
        });
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn open_from_address(&mut self, text: &str) {
        self.waiting = Some(text.to_owned());
    }

    pub(crate) fn paste(&mut self, text: &str, base: &str) {
        self.open = true;
        self.connected = None;
        let Some(link) = InviteLink::parse_contact(text) else {
            self.error = Some(
                "That is not a contact link. Copy the whole link from the person who sent it."
                    .to_owned(),
            );
            return;
        };
        if !link.is_for(base) {
            self.error = Some(format!(
                "This contact link is for {}. Switch to an account on that server to use it.",
                link.server.unwrap_or_default()
            ));
            return;
        }
        let sender = self.replies.0.clone();
        let code = link.code.clone();
        self.busy = true;
        self.error = None;
        crate::be::open_contact_link(link.code, move |opened| {
            let _ = sender.send(Reply::Opened(code, opened));
            crate::host::wake();
        });
    }

    pub(crate) fn accept(&mut self) {
        let Some(offered) = self.offered.take() else {
            return;
        };
        let sender = self.replies.0.clone();
        self.busy = true;
        crate::be::accept_contact_link(offered.code, move |accepted| {
            let _ = sender.send(Reply::Accepted(accepted));
            crate::host::wake();
        });
    }

    pub(crate) fn decline(&mut self) {
        self.offered = None;
    }

    pub(crate) fn ask_remove(&mut self, account: Uuid) {
        self.removing = Some(account);
    }

    pub(crate) fn cancel_remove(&mut self) {
        self.removing = None;
    }

    pub(crate) fn confirm_remove(&mut self) {
        if let Some(account) = self.removing.take() {
            crate::be::remove_contact(account);
        }
    }

    pub(crate) fn publish(&mut self) {
        let seen = Some((crate::be::identity(), crate::be::contacts_revision()));
        if self.published == seen {
            return;
        }
        self.published = seen;
        let contacts: Vec<block_plugin_api::HostContact> = crate::be::contacts()
            .into_iter()
            .map(|contact| block_plugin_api::HostContact {
                account: contact.account.into_bytes(),
                display_name: contact.display_name,
                email: contact.email,
                verified: contact.public.is_some(),
            })
            .collect();
        crate::plugin_host::publish::<block_plugin_api::ContactsWatch>(&contacts);
    }

    pub(crate) fn poll(&mut self, base: &str, ready: bool) {
        if ready && let Some(text) = self.waiting.take() {
            self.paste(&text, base);
        }
        while let Ok(reply) = self.replies.1.try_recv() {
            self.busy = false;
            match reply {
                Reply::Link(Ok(link)) => self.link = Some(link),
                Reply::Opened(code, Ok(card)) => self.offered = Some(Offered { code, card }),
                Reply::Accepted(Ok(card)) => self.connected = Some(described(&card)),
                Reply::Link(Err(error))
                | Reply::Opened(_, Err(error))
                | Reply::Accepted(Err(error)) => self.error = Some(error),
            }
        }
    }

    pub(crate) fn view(&self) -> Option<ContactsView> {
        if !self.open {
            return None;
        }
        let contacts = crate::be::contacts();
        let removing = self.removing.and_then(|account| {
            contacts
                .iter()
                .find(|contact| contact.account == account)
                .map(|contact| {
                    format!(
                        "Remove {}? Whatever you and they have shared with each other is \
                         unshared.",
                        named(&contact.display_name, &contact.email)
                    )
                })
        });
        Some(ContactsView {
            contacts: contacts
                .into_iter()
                .map(|contact| ContactRow {
                    account: contact.account,
                    name: contact.display_name,
                    email: contact.email,
                    verified: contact.public.is_some(),
                })
                .collect(),
            link: self.link.clone(),
            offer: self
                .offered
                .as_ref()
                .map(|offered| format!("Connect with {}?", described(&offered.card))),
            connected: self
                .connected
                .as_ref()
                .map(|name| format!("You and {name} are now contacts.")),
            removing,
            busy: self.busy,
            error: self.error.clone(),
        })
    }
}

fn named(name: &str, email: &str) -> String {
    match name.trim().is_empty() {
        true => email.to_owned(),
        false => name.to_owned(),
    }
}

fn described(card: &ContactCard) -> String {
    if card.email.is_empty() {
        return card.display_name.clone();
    }
    format!(
        "{} ({})",
        named(&card.display_name, &card.email),
        card.email
    )
}
