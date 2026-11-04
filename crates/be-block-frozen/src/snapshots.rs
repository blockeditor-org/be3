mod s2026_11_04;

/// What opens one zipped server snapshot: the tokens, recovery phrases, link and invite codes and
/// keys the `freeze` example printed into `sNAME.rs`. A new snapshot is listed in `SNAPSHOTS`.
pub struct Snapshot {
    pub name: &'static str,
    pub organization: &'static str,
    pub personal: &'static str,
    pub owner_token: &'static str,
    pub owner_phrase: &'static str,
    pub editor_phrase: &'static str,
    pub friend_token: &'static str,
    pub friend_phrase: &'static str,
    pub agent_token: &'static str,
    pub agent_phrase: &'static str,
    pub link_code: &'static str,
    pub invite_code: &'static str,
    pub backup_key: &'static str,
    pub my_files: &'static str,
    pub team: &'static str,
    pub notes: &'static str,
    pub shopping: &'static str,
    pub program: &'static str,
    pub archive: &'static str,
    pub long: &'static str,
    pub view: &'static str,
    pub database: &'static str,
    pub plan: &'static str,
    pub personal_files: &'static str,
    pub holiday: &'static str,
    pub holiday_note: &'static str,
    pub recipes: &'static str,
    pub usage: Usage,
    pub files: &'static [u8],
}

/// The storage accounting a snapshot holds, as the server counted it when the snapshot was taken.
pub struct Usage {
    /// An invite code that made one account, its only one.
    pub used_invite: &'static str,
    /// An invite code that may still make one account.
    pub unused_invite: &'static str,
    pub agent_note: &'static str,
    pub owner_held: u64,
    pub owner_pending: u64,
    pub my_files_tree_size: u64,
    pub team_size: u64,
    pub team_tree_size: u64,
}

pub const SNAPSHOTS: &[&Snapshot] = &[&s2026_11_04::SNAPSHOT];
