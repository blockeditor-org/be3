use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, LinkAdmits, ShareLink};
use uuid::Uuid;

/// What the share dialog offers. `KnowExists` is not among them: a reference
/// gives it, and a grant of it is never what a person sharing means.
pub(crate) const GRANTABLE: [AccessLevel; 3] =
    [AccessLevel::Manage, AccessLevel::Edit, AccessLevel::View];

/// What a share link offers: never Manage, which a link holder could then hand on; Join only on
/// a hosted block.
const LINK_GRANTABLE: [AccessLevel; 3] = [AccessLevel::Edit, AccessLevel::View, AccessLevel::Join];

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
    pub(crate) inherited: AccessLevel,
    pub(crate) removable: bool,
    /// Whether the account viewing owns the block and may give it to this member: someone
    /// else with access, a confirmed contact.
    pub(crate) may_own: bool,
}

impl Member {
    /// The levels this member's row offers: those the dialog grants, at or
    /// above what they inherit, and the one they hold if the dialog doesn't
    /// grant it (a `KnowExists` made by another route), so the row shows it.
    pub(crate) fn choices(&self) -> Vec<AccessLevel> {
        let mut choices: Vec<AccessLevel> = GRANTABLE
            .into_iter()
            .filter(|level| *level >= self.inherited)
            .collect();
        if self.access > AccessLevel::None && !choices.contains(&self.access) {
            choices.push(self.access);
        }
        choices
    }

    pub(crate) fn choose(&self, access: AccessLevel) -> Option<AccessLevel> {
        match access {
            _ if access == self.access => None,
            _ if access <= self.inherited && self.removable => Some(AccessLevel::None),
            _ if access <= self.inherited => None,
            _ => Some(access),
        }
    }
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
    /// Asks to confirm giving ownership to this member.
    OfferOwnership(Uuid),
    CancelOwnership,
    /// Gives ownership to the member offered it (`Share::transferring`).
    ConfirmOwnership,
    /// What the next share link grants.
    LinkAccess(AccessLevel),
    /// Whom the next share link admits.
    LinkAdmits(LinkAdmits),
    /// Makes a share link with the level and admits chosen.
    MakeLink,
    /// Asks to confirm revoking the share link with this id.
    OfferRevoke(String),
    CancelRevoke,
    /// Revokes the share link offered (`Share::revoking`).
    ConfirmRevoke,
}

/// What the share dialog holds besides the listing of who has access, which the workspace
/// asks the host for (`Workspace::share_listing`) and hands to what needs it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Share {
    pub(crate) block: Uuid,
    pub(crate) error: Option<String>,
    pub(crate) query: String,
    /// Enter was pressed before the members were listed: the listing carries it out.
    pub(crate) submit_waiting: bool,
    pub(crate) pending: Vec<Person>,
    pub(crate) pending_access: AccessLevel,
    pub(crate) inviting: Option<u64>,
    pub(crate) invite_link: Option<String>,
    /// The member whom the person asked to give ownership to, until they confirm or cancel.
    pub(crate) transferring: Option<Uuid>,
    /// What the next share link grants, once the person chose (`Share::link_access`), and
    /// whom it admits.
    pub(crate) chosen_link_access: Option<AccessLevel>,
    pub(crate) link_admits: LinkAdmits,
    /// The share link being made, by ask, until the host answers.
    pub(crate) making_link: Option<u64>,
    /// The share link the person asked to revoke, until they confirm or cancel.
    pub(crate) revoking: Option<String>,
    /// The share links whose revocation the host has not answered yet.
    pub(crate) revoking_now: Vec<String>,
}

impl Share {
    pub(crate) fn new(block: Uuid) -> Self {
        Self {
            block,
            error: None,
            query: String::new(),
            submit_waiting: false,
            pending: Vec::new(),
            pending_access: AccessLevel::Edit,
            inviting: None,
            invite_link: None,
            transferring: None,
            chosen_link_access: None,
            link_admits: LinkAdmits::Members,
            making_link: None,
            revoking: None,
            revoking_now: Vec::new(),
        }
    }

    /// Carries out an Enter pressed while the members were still being listed.
    pub(crate) fn listed(&mut self, listing: &AccessListing, me: Uuid) {
        if std::mem::take(&mut self.submit_waiting) {
            self.submit(listing, me);
        }
    }

    fn submit(&mut self, listing: &AccessListing, me: Uuid) {
        if let Some(person) = self.candidates(listing, me).first().cloned() {
            self.pending.push(person);
            self.query.clear();
        }
    }

    /// What `action` does, given the listing if it has arrived: the grants to make, and
    /// whether to list again.
    pub(crate) fn act(
        &mut self,
        action: ShareAction,
        listing: Option<&AccessListing>,
        me: Uuid,
    ) -> (Vec<(Uuid, AccessLevel)>, bool) {
        let mut grants = Vec::new();
        let mut reload = false;
        match action {
            ShareAction::Query(query) => {
                if query != self.query {
                    self.submit_waiting = false;
                }
                self.query = query;
            }
            ShareAction::Submit => match listing {
                Some(listing) => self.submit(listing, me),
                None => self.submit_waiting = !self.query.trim().is_empty(),
            },
            ShareAction::Pick(id) => {
                if let Some(person) = listing
                    .map(|listing| self.candidates(listing, me))
                    .unwrap_or_default()
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
            ShareAction::SetAccess(account, access) => {
                if listing.is_some_and(|listing| {
                    listing
                        .grants
                        .iter()
                        .any(|grant| grant.account == account.into_bytes())
                }) {
                    grants.push((account, access));
                }
            }
            ShareAction::Refresh => reload = true,
            ShareAction::OfferOwnership(account) => {
                let offered = listing.is_some_and(|listing| {
                    self.members(listing, me, |_| String::new())
                        .iter()
                        .any(|member| member.id == account && member.may_own)
                });
                if offered {
                    self.transferring = Some(account);
                }
            }
            ShareAction::CancelOwnership => self.transferring = None,
            ShareAction::LinkAccess(access) => self.chosen_link_access = Some(access),
            ShareAction::LinkAdmits(admits) => self.link_admits = admits,
            ShareAction::OfferRevoke(link) => self.revoking = Some(link),
            ShareAction::CancelRevoke => self.revoking = None,
            ShareAction::InviteNew
            | ShareAction::ConfirmOwnership
            | ShareAction::MakeLink
            | ShareAction::ConfirmRevoke => {}
        }
        (grants, reload)
    }

    pub(crate) fn suggestions(&self, listing: &AccessListing, me: Uuid) -> Option<Vec<Person>> {
        (!self.query.is_empty()).then(|| {
            self.candidates(listing, me)
                .into_iter()
                .take(MAX_SUGGESTIONS)
                .collect()
        })
    }

    pub(crate) fn members(
        &self,
        listing: &AccessListing,
        me: Uuid,
        name_of: impl Fn(Uuid) -> String,
    ) -> Vec<Member> {
        let owner = listing
            .grants
            .iter()
            .any(|grant| grant.author && grant.account == me.into_bytes());
        listing
            .grants
            .iter()
            .filter(|grant| has_access(grant, me))
            .map(|grant| member(grant, me, self.block, owner, &name_of))
            .collect()
    }

    /// The levels a share link on the block offers: Join only on a hosted block, which offers it
    /// first.
    pub(crate) fn link_levels(listing: &AccessListing) -> Vec<AccessLevel> {
        let mut levels: Vec<AccessLevel> = LINK_GRANTABLE
            .into_iter()
            .filter(|level| *level != AccessLevel::Join || listing.hosted)
            .collect();
        if listing.hosted {
            levels.rotate_right(1);
        }
        levels
    }

    /// What the next share link grants: what the person chose, else Join on a hosted block
    /// (plans/hosted_blocks.md, "The share dialog") and View on any other.
    pub(crate) fn link_access(&self, listing: Option<&AccessListing>) -> AccessLevel {
        self.chosen_link_access.unwrap_or(match listing.is_some_and(|listing| listing.hosted) {
            true => AccessLevel::Join,
            false => AccessLevel::View,
        })
    }

    /// Whether `me` manages the block as `listing` lists it: only a manager sees and makes its
    /// share links.
    pub(crate) fn manages(listing: &AccessListing, me: Uuid) -> bool {
        listing
            .grants
            .iter()
            .any(|grant| grant.account == me.into_bytes() && grant.effective == AccessLevel::Manage)
    }

    /// Whom links may admit: the organization's policy, and in a company organization a
    /// choice between its members and anyone with an account on the server.
    pub(crate) fn link_choices(listing: &AccessListing) -> Vec<LinkAdmits> {
        match (listing.links_admit, listing.company) {
            (None, _) => Vec::new(),
            (Some(LinkAdmits::Anyone), true) => vec![LinkAdmits::Members, LinkAdmits::Anyone],
            (Some(_), _) => vec![LinkAdmits::Members],
        }
    }

    pub(crate) fn admits_label(admits: LinkAdmits) -> &'static str {
        match admits {
            LinkAdmits::Members => "People in the organization",
            LinkAdmits::Anyone => "Anyone with an account here",
        }
    }

    /// What a share link lets whoever opens it do, in a sentence.
    pub(crate) fn describe_link(link: &ShareLink, company: bool, me: Uuid) -> String {
        let who = match (link.admits, company) {
            (LinkAdmits::Members, true) => "Anyone in the organization with the link",
            (LinkAdmits::Anyone, true) => "Anyone with the link and an account on this server",
            (_, false) => "Anyone with the link",
        };
        let what = match link.access {
            AccessLevel::Edit => "can edit",
            AccessLevel::Join => "can join",
            _ => "can view",
        };
        match (link.maker == me.into_bytes(), link.maker_name.is_empty()) {
            (true, _) => format!("{who} {what}. Made by you."),
            (false, true) => format!("{who} {what}."),
            (false, false) => format!("{who} {what}. Made by {}.", link.maker_name),
        }
    }

    pub(crate) fn inside_note(listing: &AccessListing) -> Option<String> {
        match listing.set_inside {
            0 => None,
            1 => Some("1 item inside has different access".to_owned()),
            count => Some(format!("{count} items inside have different access")),
        }
    }

    fn candidates(&self, listing: &AccessListing, me: Uuid) -> Vec<Person> {
        let query = self.query.trim().to_lowercase();
        listing
            .grants
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

fn member(
    grant: &AccessGrant,
    me: Uuid,
    block: Uuid,
    owner: bool,
    name_of: &impl Fn(Uuid) -> String,
) -> Member {
    let fixed = match (
        grant.administrator,
        grant.account == me.into_bytes(),
        grant.author,
    ) {
        (true, _, _) => Some("Administrators can open every block"),
        (false, true, _) => Some("This is you"),
        (false, false, true) => Some("Owns this block"),
        (false, false, false) => None,
    };
    let source = grant.source.map(Uuid::from_bytes);
    let note = match (grant.administrator, grant.waiting && fixed.is_none()) {
        (true, _) => None,
        (false, true) => {
            Some("Opens for them once you have both confirmed each other as contacts".to_owned())
        }
        (false, false) if grant.author && source == Some(block) => None,
        (false, false) if grant.through_link && source == Some(block) => {
            Some(format!("{}, through a share link", grant.effective.label()))
        }
        (false, false) => origin(grant.effective, source, block, name_of),
    };
    // Someone there only through a link is no one to hand the block to: a guest, or a member
    // whose access is the link's alone.
    let may_own = owner
        && grant.account != me.into_bytes()
        && !grant.author
        && !grant.guest
        && !grant.through_link
        && !grant.waiting
        && has_access(grant, me);
    Member {
        may_own,
        id: Uuid::from_bytes(grant.account),
        name: grant.display_name.clone(),
        email: grant.email.clone(),
        fixed: fixed.map(str::to_owned),
        note,
        access: match fixed {
            Some(_) => grant.effective,
            None => grant
                .effective
                .max(grant.granted.unwrap_or(AccessLevel::None)),
        },
        inherited: grant.inherited,
        removable: fixed.is_none()
            && grant
                .granted
                .is_some_and(|granted| granted != AccessLevel::None),
    }
}

fn origin(
    effective: AccessLevel,
    source: Option<Uuid>,
    block: Uuid,
    name_of: &impl Fn(Uuid) -> String,
) -> Option<String> {
    let level = effective.label();
    match source {
        _ if effective == AccessLevel::None => None,
        Some(source) if source == block => Some(format!("{level}, set here")),
        Some(source) => Some(format!("{level}, from {}", name_of(source))),
        None => Some(format!("{level}, through a link")),
    }
}

fn has_access(grant: &AccessGrant, me: Uuid) -> bool {
    grant.administrator
        || grant.account == me.into_bytes()
        || grant.granted.is_some()
        || grant.effective > AccessLevel::None
}
