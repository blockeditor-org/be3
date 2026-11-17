//! Who is in a block's live session, and so who holds its lease: the server writes membership
//! entries into the session's order (joining, leaving, a change of edit access, a claim), and
//! the server and every replica derive the holder from them here, alike.

use be_block_protocol::{
    ClientId, JoinedParticipant, MembershipChange, MembershipEntry, Participant, SessionState,
};
use be_block_store::Hash;

/// What a session's membership answers, over the `SessionState` the server and replicas hold.
pub trait Membership {
    /// Applies `entry` when it is the next one (its index this one's plus one); answers
    /// whether it did. One held already changes nothing, and so does one past a gap, which a
    /// participant hearing one stream in order never sees: it joins again rather than guess.
    fn apply(&mut self, entry: &MembershipEntry) -> bool;

    /// Writes the next entry, about `client`, stamped `after` the order's last number, and
    /// applies it; none when it would change nothing (leaving a session it is not in, or an
    /// edit access it has already).
    fn write(
        &mut self,
        after: u64,
        client: ClientId,
        change: MembershipChange,
    ) -> Option<MembershipEntry>;

    /// The lease holder, who seals: of the participants with edit access that run the build
    /// the session speaks, the last claimant, else the earliest joined. None while there is no
    /// such participant. One of another build never holds it, since it could not apply the
    /// session's entries, and with no expiry it would otherwise keep it.
    fn holder(&self) -> Option<ClientId>;

    fn is_holder(&self, client: ClientId) -> bool {
        self.holder() == Some(client)
    }

    /// The build of the block's type the session speaks: its first participant's.
    fn tag(&self) -> Option<Hash>;

    /// Whether `client` is a participant running the build the session speaks.
    fn speaks(&self, client: ClientId) -> bool;

    /// Whether `client` is a participant.
    fn has(&self, client: ClientId) -> bool;

    /// The participants, in the order they joined.
    fn clients(&self) -> Vec<ClientId>;
}

impl Membership for SessionState {
    fn apply(&mut self, entry: &MembershipEntry) -> bool {
        if entry.index != self.index + 1 {
            return false;
        }
        self.index = entry.index;
        self.after = entry.after;
        let held = self
            .participants
            .iter()
            .position(|participant| participant.client == entry.client);
        match (&entry.change, held) {
            (MembershipChange::Joined(JoinedParticipant { tag, may_edit }), Some(at)) => {
                let participant = &mut self.participants[at];
                participant.tag = *tag;
                participant.may_edit = *may_edit;
            }
            (MembershipChange::Joined(JoinedParticipant { tag, may_edit }), None) => {
                self.participants.push(Participant {
                    client: entry.client,
                    tag: *tag,
                    may_edit: *may_edit,
                    claimed: None,
                });
            }
            (MembershipChange::Left, Some(at)) => {
                self.participants.remove(at);
            }
            (MembershipChange::Access(may_edit), Some(at)) => {
                self.participants[at].may_edit = *may_edit;
            }
            (MembershipChange::Claimed, Some(at)) => {
                self.participants[at].claimed = Some(entry.index);
            }
            (_, None) => {}
        }
        true
    }

    fn write(
        &mut self,
        after: u64,
        client: ClientId,
        change: MembershipChange,
    ) -> Option<MembershipEntry> {
        let held = self
            .participants
            .iter()
            .find(|participant| participant.client == client);
        let changes = match (&change, held) {
            (MembershipChange::Joined(_), _) => true,
            (MembershipChange::Access(may_edit), Some(held)) => held.may_edit != *may_edit,
            (MembershipChange::Left | MembershipChange::Claimed, Some(_)) => true,
            (_, None) => false,
        };
        if !changes {
            return None;
        }
        let entry = MembershipEntry {
            index: self.index + 1,
            after,
            client,
            change,
        };
        self.apply(&entry);
        Some(entry)
    }

    fn holder(&self) -> Option<ClientId> {
        let tag = self.tag();
        let editors = || {
            self.participants
                .iter()
                .filter(move |participant| participant.may_edit && Some(participant.tag) == tag)
        };
        editors()
            .filter(|participant| participant.claimed.is_some())
            .max_by_key(|participant| participant.claimed)
            .or_else(|| editors().next())
            .map(|participant| participant.client)
    }

    fn tag(&self) -> Option<Hash> {
        self.participants.first().map(|participant| participant.tag)
    }

    fn speaks(&self, client: ClientId) -> bool {
        self.participants
            .iter()
            .any(|participant| participant.client == client && Some(participant.tag) == self.tag())
    }

    fn has(&self, client: ClientId) -> bool {
        self.participants
            .iter()
            .any(|participant| participant.client == client)
    }

    fn clients(&self) -> Vec<ClientId> {
        self.participants
            .iter()
            .map(|participant| participant.client)
            .collect()
    }
}
