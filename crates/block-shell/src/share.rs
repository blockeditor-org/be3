use block_editor_beui::beui::reactive::{Pending, Ready};
use block_editor_beui::{AccessGrant, AccessLevel, Failure};
use uuid::Uuid;

pub(crate) const GRANTABLE: [AccessLevel; 3] = [
    AccessLevel::Edit,
    AccessLevel::View,
    AccessLevel::KnowExists,
];

const MAX_SUGGESTIONS: usize = 6;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Person {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) email: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Member {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) fixed: Option<String>,
    pub(crate) note: Option<String>,
    pub(crate) access: AccessLevel,
    pub(crate) removable: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum ShareAction {
    Query(String),
    Submit,
    Pick(Uuid),
    Unpick(Uuid),
    PendingAccess(AccessLevel),
    AddPending,
    SetAccess(Uuid, AccessLevel),
    Refresh,
    InviteNew,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Share {
    pub(crate) block: Uuid,
    pub(crate) request: Option<u64>,
    pub(crate) grants: Vec<AccessGrant>,
    pub(crate) members: Ready<()>,
    pub(crate) error: Option<String>,
    pub(crate) query: String,
    pub(crate) pending: Vec<Person>,
    pub(crate) pending_access: AccessLevel,
    pub(crate) inviting: Option<u64>,
    pub(crate) invite_link: Option<String>,
}

impl Share {
    pub(crate) fn new(block: Uuid, request: u64) -> Self {
        Self {
            block,
            request: Some(request),
            grants: Vec::new(),
            members: Err(Pending::Loading),
            error: None,
            query: String::new(),
            pending: Vec::new(),
            pending_access: AccessLevel::Edit,
            inviting: None,
            invite_link: None,
        }
    }

    pub(crate) fn listed(&mut self, listing: Result<Vec<AccessGrant>, Failure>) {
        self.request = None;
        match listing {
            Ok(grants) => {
                self.grants = grants;
                self.members = Ok(());
            }
            Err(error) => self.members = Err(Pending::Failed(error.to_string())),
        }
    }

    pub(crate) fn relisting(&mut self) {
        if self.members.is_err() {
            self.members = Err(Pending::Loading);
        }
    }

    pub(crate) fn act(
        &mut self,
        action: ShareAction,
        me: Uuid,
    ) -> (Vec<(Uuid, AccessLevel)>, bool) {
        let mut grants = Vec::new();
        let mut reload = false;
        match action {
            ShareAction::Query(query) => self.query = query,
            ShareAction::Submit => {
                if let Some(person) = self.candidates(me).first().cloned() {
                    self.pending.push(person);
                    self.query.clear();
                }
            }
            ShareAction::Pick(id) => {
                if let Some(person) = self
                    .candidates(me)
                    .into_iter()
                    .find(|person| person.id == id)
                {
                    self.pending.push(person);
                    self.query.clear();
                }
            }
            ShareAction::Unpick(id) => self.pending.retain(|person| person.id != id),
            ShareAction::PendingAccess(access) => self.pending_access = access,
            ShareAction::AddPending => {
                let access = self.pending_access;
                grants.extend(self.pending.drain(..).map(|person| (person.id, access)));
                self.query.clear();
            }
            ShareAction::SetAccess(account, access) => grants.push((account, access)),
            ShareAction::Refresh => reload = true,
            ShareAction::InviteNew => {}
        }
        (grants, reload)
    }

    pub(crate) fn suggestions(&self, me: Uuid) -> Option<Vec<Person>> {
        (!self.query.is_empty()).then(|| {
            self.candidates(me)
                .into_iter()
                .take(MAX_SUGGESTIONS)
                .collect()
        })
    }

    pub(crate) fn members(&self, me: Uuid) -> Vec<Member> {
        self.grants
            .iter()
            .filter(|grant| has_access(grant, me))
            .map(|grant| member(grant, me))
            .collect()
    }

    fn candidates(&self, me: Uuid) -> Vec<Person> {
        let query = self.query.trim().to_lowercase();
        self.grants
            .iter()
            .filter(|grant| !has_access(grant, me))
            .filter(|grant| {
                !self
                    .pending
                    .iter()
                    .any(|pending| pending.id.into_bytes() == grant.account)
            })
            .filter(|grant| {
                query.is_empty()
                    || grant.display_name.to_lowercase().contains(&query)
                    || grant.email.to_lowercase().contains(&query)
            })
            .map(|grant| Person {
                id: Uuid::from_bytes(grant.account),
                name: grant.display_name.clone(),
                email: grant.email.clone(),
            })
            .collect()
    }
}

fn member(grant: &AccessGrant, me: Uuid) -> Member {
    let fixed = match (
        grant.administrator,
        grant.account == me.into_bytes(),
        grant.author,
    ) {
        (true, _, _) => Some("Administrators can open every block"),
        (false, true, _) => Some("This is you"),
        (false, false, true) => Some("Made this block"),
        (false, false, false) => None,
    };
    Member {
        id: Uuid::from_bytes(grant.account),
        name: grant.display_name.clone(),
        email: grant.email.clone(),
        fixed: fixed.map(str::to_owned),
        note: match fixed {
            Some(_) => None,
            None if grant.waiting => Some(
                "Opens for them once you have both confirmed each other as contacts".to_owned(),
            ),
            None => (grant.granted != Some(grant.effective))
                .then(|| format!("Inherited: {}", grant.effective.label())),
        },
        access: match fixed {
            Some(_) => grant.effective,
            None => grant.granted.unwrap_or(AccessLevel::None),
        },
        removable: fixed.is_none() && grant.granted != Some(AccessLevel::None),
    }
}

fn has_access(grant: &AccessGrant, me: Uuid) -> bool {
    grant.administrator
        || grant.account == me.into_bytes()
        || grant.granted.is_some()
        || grant.effective > AccessLevel::None
}
