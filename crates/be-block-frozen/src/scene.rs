//! What the `freeze` example writes into a real server before it zips the data directory and a
//! backup into `src/snapshots/NAME.zip`: a company organization (My files with notes, a Team
//! workspace holding a tracker database shared with an agent, an administrator demoted so the
//! escrow key rotated, a grant revoked so Team's scope key rotated, a pending invitation, and
//! the organization's settings shared with the server, holding a policy it enforces) and the personal organization (a folder shared with a contact made by link, a
//! pending link carrying a grant, and that contact's change of recovery phrase).
//!
//! Once a snapshot is taken, these values are what old data looks like: changing one stops the
//! snapshots' test guarding that data. Add to the scene, with a `*_SINCE` snapshot name, rather
//! than edit it. Nothing is frozen now (guides/frozen_formats.md), so `SNAPSHOTS` is empty and
//! the scene may change freely.

pub const ORGANIZATION: &str = "Frozen";

pub const OWNER_EMAIL: &str = "owner@example.com";
pub const OWNER_NAME: &str = "Owner";
pub const OWNER_PASSWORD: &str = "correct horse battery staple";

pub const EDITOR_EMAIL: &str = "editor@example.com";
pub const EDITOR_NAME: &str = "Editor";
pub const EDITOR_PASSWORD: &str = "tr0ub4dor and three";

pub const FRIEND_EMAIL: &str = "friend@example.com";
pub const FRIEND_NAME: &str = "Friend";
pub const FRIEND_PASSWORD: &str = "a friend's password";

pub const INVITED_EMAIL: &str = "invited@example.com";

pub const AGENT_NAME: &str = "Tracker agent";

pub const MY_FILES: &str = "My files";
pub const TEAM: &str = "Team";
pub const PLAN: [&str; 2] = [
    "Plan, as the editor saw it\n",
    "Plan, after the editor left\n",
];
pub const HOLIDAY: &str = "Holiday";
pub const HOLIDAY_NOTE: &str = "Tickets are in the drawer.\n";
pub const RECIPES: &str = "Pancakes: flour, eggs, milk.\n";

pub const NOTES: &str = "Notes";
pub const ARCHIVE: &str = "Archive";

pub const SHOPPING: [&str; 3] = [
    "Shopping\n\n- eggs\n",
    "Shopping\n\n- eggs\n- flour\n",
    "Shopping\n\n- eggs\n- flour\n- milk, the oat kind ✓\n",
];

pub const PROGRAM: &str = "fn main() {\n\tprintln!(\"héllo, 世界\");\n}\n";

pub const DATABASE: &str = "Issues";
pub const PREFIX: &str = "ISS";
pub const STATUSES: [&str; 3] = ["To do", "In progress", "Done"];
pub const FIRST_ROW: &str = "Freeze the formats";
pub const FIRST_PAGE: &str = "Text, folders, databases and the server.\n";
pub const SECOND_ROW: &str = "Read old data forever";

pub fn long_text() -> String {
    (1..=200)
        .map(|line| format!("line {line}: a text long enough to be split into many chunks\n"))
        .collect()
}
