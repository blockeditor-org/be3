use std::sync::mpsc::{Receiver, Sender, channel};

use be_protocol::{Member, OrganizationRole};
use uuid::Uuid;

use crate::ui::{MemberRow, MembersView};

type Reply = Result<Vec<Member>, String>;

pub(crate) struct Members {
    open: bool,
    members: Vec<Member>,
    removing: Option<Uuid>,
    busy: bool,
    error: Option<String>,
    replies: (Sender<Reply>, Receiver<Reply>),
}

impl Default for Members {
    fn default() -> Self {
        Self {
            open: false,
            members: Vec::new(),
            removing: None,
            busy: false,
            error: None,
            replies: channel(),
        }
    }
}

impl Members {
    fn answer(&self) -> Box<dyn FnOnce(Reply) + Send> {
        let sender = self.replies.0.clone();
        Box::new(move |reply| {
            let _ = sender.send(reply);
            block_app_base::host::wake();
        })
    }

    pub(crate) fn show(&mut self) {
        self.open = true;
        self.error = None;
        self.busy = true;
        block_app_stack::list_members(self.answer());
    }

    pub(crate) fn close(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn set_role(&mut self, account: Uuid, role: OrganizationRole) {
        self.busy = true;
        self.error = None;
        block_app_stack::set_role(account, role, self.answer());
    }

    pub(crate) fn ask_remove(&mut self, account: Uuid) {
        self.removing = Some(account);
    }

    pub(crate) fn cancel_remove(&mut self) {
        self.removing = None;
    }

    pub(crate) fn confirm_remove(&mut self) {
        if let Some(account) = self.removing.take() {
            self.busy = true;
            self.error = None;
            block_app_stack::remove_member(account, self.answer());
        }
    }

    pub(crate) fn poll(&mut self) {
        while let Ok(reply) = self.replies.1.try_recv() {
            self.busy = false;
            match reply {
                Ok(members) => self.members = members,
                Err(error) => self.error = Some(error),
            }
        }
    }

    pub(crate) fn view(&self, me: Uuid) -> Option<MembersView> {
        if !self.open {
            return None;
        }
        let removing = self.removing.and_then(|account| {
            self.members
                .iter()
                .find(|member| member.account == account)
                .map(|member| {
                    format!(
                        "Remove {} from this organization? They lose everything shared with \
                         them here, and what they could open is given new keys.",
                        named(member)
                    )
                })
        });
        Some(MembersView {
            members: self
                .members
                .iter()
                .map(|member| MemberRow {
                    account: member.account,
                    name: named(member),
                    detail: match (member.agent, member.role) {
                        (true, _) => format!("{}, agent", member.email),
                        (false, role) if member.email.is_empty() => role.label().to_owned(),
                        (false, role) => format!("{}, {}", member.email, role.label()),
                    },
                    administrator: member.role == OrganizationRole::Administrator,
                    me: member.account == me,
                    agent: member.agent,
                })
                .collect(),
            removing,
            busy: self.busy,
            error: self.error.clone(),
        })
    }
}

fn named(member: &Member) -> String {
    match member.display_name.trim().is_empty() {
        true => member.email.clone(),
        false => member.display_name.clone(),
    }
}
