/// What opens one zipped server snapshot: the tokens, recovery phrases, link and invite codes and
/// keys the `freeze` example printed into `sNAME.rs`. A new snapshot is listed in `SNAPSHOTS`;
/// the test checks each part of the scene only in the snapshots that hold it.
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
    pub files: &'static [u8],
}

/// Empty until the next baseline: the server's schema changed in a way the earlier snapshots
/// cannot open, which the release that wipes the deployed server allows once.
pub const SNAPSHOTS: &[&Snapshot] = &[];
