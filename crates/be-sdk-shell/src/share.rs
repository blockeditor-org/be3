use be_sdk_beui::be_block::LinkSharing;
use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, LinkAdmits, OfferedLevel, ShareLink};
use uuid::Uuid;

/// What the share dialog offers. `KnowExists` is not among them: a reference
/// gives it, and a grant of it is never what a person sharing means.
const GRANTABLE: [AccessLevel; 3] = [AccessLevel::Manage, AccessLevel::Edit, AccessLevel::View];

/// What a share link offers on a block that is not hosted: never Manage, which a link holder
/// could then hand on, nor Join, which only a hosted block offers.
const LINK_GRANTABLE: [AccessLevel; 2] = [AccessLevel::Edit, AccessLevel::View];

/// The levels the share dialog offers on a block, and what it calls each: a hosted type's own
/// levels, names and lines from its manifest (plans/hosted_blocks.md, "The share dialog"), or
/// the core levels by their own names.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Offered(Vec<OfferedLevel>);

impl Offered {
    pub(crate) fn of(listing: &AccessListing) -> Self {
        Self(listing.levels.clone())
    }

    /// Whether the block is hosted, so it offers its type's levels.
    pub(crate) fn hosted(&self) -> bool {
        !self.0.is_empty()
    }

    /// What the dialog grants people, from the most to the least, as every level list in it
    /// runs: the order a manifest lists its levels in counts for nothing.
    pub(crate) fn grantable(&self) -> Vec<AccessLevel> {
        match self.hosted() {
            true => {
                let mut levels: Vec<AccessLevel> =
                    self.0.iter().map(|level| level.access).collect();
                levels.sort_by(|a, b| b.cmp(a));
                levels
            }
            false => GRANTABLE.to_vec(),
        }
    }

    /// What a share link offers: never Manage; on a hosted block its levels with Join first.
    pub(crate) fn link_levels(&self) -> Vec<AccessLevel> {
        if !self.hosted() {
            return LINK_GRANTABLE.to_vec();
        }
        let mut levels: Vec<AccessLevel> = self
            .grantable()
            .into_iter()
            .filter(|level| *level != AccessLevel::Manage)
            .collect();
        levels.sort_by_key(|level| *level != AccessLevel::Join);
        levels
    }

    /// What the dialog calls `level`: the type's name for it, or the core one.
    pub(crate) fn name(&self, level: AccessLevel) -> String {
        self.0
            .iter()
            .find(|offered| offered.access == level)
            .map_or_else(|| level.label().to_owned(), |offered| offered.name.clone())
    }

    /// The type's line saying what `level` means; empty for the core levels.
    pub(crate) fn line(&self, level: AccessLevel) -> String {
        self.0
            .iter()
            .find(|offered| offered.access == level)
            .map(|offered| offered.line.clone())
            .unwrap_or_default()
    }

    /// Where the dialog starts: Join on a hosted block, the least it offers; else `core`.
    fn first(&self, core: AccessLevel) -> AccessLevel {
        match self.hosted() {
            true => AccessLevel::Join,
            false => core,
        }
    }
}

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
    pub(crate) fn choices(&self, offered: &Offered) -> Vec<AccessLevel> {
        let mut choices: Vec<AccessLevel> = offered
            .grantable()
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

/// An offer of a block's ownership, as the share dialog shows it (`Share::ownership_offer`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OwnershipOffer {
    /// Its owner, named, offered it to the person.
    ToMe { from: String },
    /// The person, its owner, offered it to the member named.
    ByMe { to: String },
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
    /// Asks to confirm offering ownership to this member.
    OfferOwnership(Uuid),
    CancelOwnership,
    /// Offers ownership to the member chosen (`Share::transferring`).
    ConfirmOwnership,
    /// Withdraws the owner's offer of ownership (`AccessListing::offered`).
    WithdrawOwnership,
    /// Accepts ownership the owner offered the person.
    AcceptOwnership,
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
    /// What the people being added are granted, once the person chose (`Share::pending_access`).
    pub(crate) chosen_access: Option<AccessLevel>,
    pub(crate) inviting: Option<u64>,
    pub(crate) invite_link: Option<String>,
    /// The member whom the person asked to offer ownership to, until they confirm or cancel.
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
            chosen_access: None,
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
            ShareAction::PendingAccess(access) => self.chosen_access = Some(access),
            ShareAction::AddPending => {
                let access = self.pending_access(listing);
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
            | ShareAction::WithdrawOwnership
            | ShareAction::AcceptOwnership
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
        let offered = Offered::of(listing);
        listing
            .grants
            .iter()
            .filter(|grant| has_access(grant, me))
            .map(|grant| member(grant, me, self.block, owner, &offered, &name_of))
            .collect()
    }

    /// What the people being added are granted: what the person chose, else Join on a hosted
    /// block and Edit on any other.
    pub(crate) fn pending_access(&self, listing: Option<&AccessListing>) -> AccessLevel {
        self.chosen_access.unwrap_or_else(|| {
            listing
                .map(Offered::of)
                .unwrap_or_default()
                .first(AccessLevel::Edit)
        })
    }

    /// What the next share link grants: what the person chose, else Join on a hosted block
    /// (plans/hosted_blocks.md, "The share dialog") and View on any other.
    pub(crate) fn link_access(&self, listing: Option<&AccessListing>) -> AccessLevel {
        self.chosen_link_access.unwrap_or_else(|| {
            listing
                .map(Offered::of)
                .unwrap_or_default()
                .first(AccessLevel::View)
        })
    }

    /// The offer of ownership standing on the block as `listing` lists it, as `me` sees it: one
    /// made to them, or one they made as its owner. Nobody else is told of it.
    pub(crate) fn ownership_offer(listing: &AccessListing, me: Uuid) -> Option<OwnershipOffer> {
        let offered = listing.offered?;
        let name = |grant: &AccessGrant| grant.display_name.clone();
        if offered == me.into_bytes() {
            let from = listing.grants.iter().find(|grant| grant.author).map(name);
            return Some(OwnershipOffer::ToMe {
                from: from.unwrap_or_default(),
            });
        }
        listing
            .grants
            .iter()
            .any(|grant| grant.author && grant.account == me.into_bytes())
            .then(|| OwnershipOffer::ByMe {
                to: listing
                    .grants
                    .iter()
                    .find(|grant| grant.account == offered)
                    .map(name)
                    .unwrap_or_default(),
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

    /// Whom links may admit: in a personal organization its members; in a company one what
    /// its policy allows, as this member reads it (the server holds makers to it too).
    pub(crate) fn link_choices(policy: LinkSharing, company: bool) -> Vec<LinkAdmits> {
        match (company, policy) {
            (false, _) => vec![LinkAdmits::Members],
            (true, LinkSharing::Off) => Vec::new(),
            (true, LinkSharing::Members) => vec![LinkAdmits::Members],
            (true, LinkSharing::Anyone) => vec![LinkAdmits::Members, LinkAdmits::Anyone],
        }
    }

    /// Whether an invitation may be made: its share link admits one person, which the server
    /// allows where the organization shares outside it, and always in a personal organization.
    pub(crate) fn invites(shares_outside: bool, company: bool) -> bool {
        !company || shares_outside
    }

    pub(crate) fn admits_label(admits: LinkAdmits) -> &'static str {
        match admits {
            LinkAdmits::Members => "People in the organization",
            LinkAdmits::Anyone => "Anyone with an account here",
            LinkAdmits::One => "The first person who opens it",
        }
    }

    /// What a share link lets whoever opens it do, in a sentence: on a hosted block, the level
    /// by its type's name.
    pub(crate) fn describe_link(
        link: &ShareLink,
        company: bool,
        me: Uuid,
        offered: &Offered,
    ) -> String {
        let who = match (link.admits, company) {
            (LinkAdmits::One, _) => "The first person to open the link",
            (LinkAdmits::Members, true) => "Anyone in the organization with the link",
            (LinkAdmits::Anyone, true) => "Anyone with the link and an account on this server",
            (_, false) => "Anyone with the link",
        };
        let what = match (offered.hosted(), link.access) {
            (true, access) => format!("can {}", offered.name(access)),
            (false, AccessLevel::Edit) => "can edit".to_owned(),
            (false, AccessLevel::Join) => "can join".to_owned(),
            (false, _) => "can view".to_owned(),
        };
        let made = match (link.maker == me.into_bytes(), link.maker_name.is_empty()) {
            (true, _) => format!("{who} {what}. Made by you."),
            (false, true) => format!("{who} {what}."),
            (false, false) => format!("{who} {what}. Made by {}.", link.maker_name),
        };
        // A link for one person says whether it was used, and by whom.
        match (link.admits, link.opened_by.as_deref()) {
            (LinkAdmits::One, None) => format!("{made} Not opened yet."),
            (LinkAdmits::One, Some("")) => format!("{made} Used."),
            (LinkAdmits::One, Some(name)) => format!("{made} Used by {name}."),
            _ => made,
        }
    }

    /// Who hosts a hosted block now, in a sentence: none for a block that is not hosted.
    pub(crate) fn hosting_note(listing: &AccessListing, me: Uuid) -> Option<String> {
        if listing.levels.is_empty() {
            return None;
        }
        let Some(host) = listing.host else {
            return Some(
                "No one is hosting it now: it runs while someone who can host has it open."
                    .to_owned(),
            );
        };
        if host == me.into_bytes() {
            return Some("You are hosting it now.".to_owned());
        }
        let name = listing
            .grants
            .iter()
            .find(|grant| grant.account == host)
            .map(|grant| grant.display_name.trim())
            .filter(|name| !name.is_empty());
        Some(match name {
            Some(name) => format!("{name} is hosting it now."),
            None => "Someone is hosting it now.".to_owned(),
        })
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
    offered: &Offered,
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
        (false, false) if grant.through_link && source == Some(block) => Some(format!(
            "{}, through a share link",
            offered.name(grant.effective)
        )),
        (false, false) => origin(grant.effective, source, block, offered, name_of),
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
    offered: &Offered,
    name_of: &impl Fn(Uuid) -> String,
) -> Option<String> {
    let level = offered.name(effective);
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
