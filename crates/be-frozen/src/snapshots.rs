mod s2026_10_12;

pub struct Snapshot {
    pub name: &'static str,
    pub workspace: &'static str,
    pub workspace_key: &'static str,
    pub owner_token: &'static str,
    pub owner_phrase: &'static str,
    pub editor_phrase: &'static str,
    pub agent_token: &'static str,
    pub agent_phrase: &'static str,
    pub backup_key: &'static str,
    pub notes: &'static str,
    pub shopping: &'static str,
    pub program: &'static str,
    pub archive: &'static str,
    pub long: &'static str,
    pub view: &'static str,
    pub database: &'static str,
    pub data: &'static [(&'static str, &'static [u8])],
    pub backup: (&'static str, &'static [u8]),
}

pub const SNAPSHOTS: &[&Snapshot] = &[&s2026_10_12::SNAPSHOT];
