mod s2026_10_16;
mod s2026_10_25;

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

pub const SNAPSHOTS: &[&Snapshot] = &[&s2026_10_16::SNAPSHOT, &s2026_10_25::SNAPSHOT];
