pub const WORKSPACE: &str = "Frozen";

pub const OWNER_EMAIL: &str = "owner@example.com";
pub const OWNER_NAME: &str = "Owner";
pub const OWNER_PASSWORD: &str = "correct horse battery staple";

pub const EDITOR_EMAIL: &str = "editor@example.com";
pub const EDITOR_NAME: &str = "Editor";
pub const EDITOR_PASSWORD: &str = "tr0ub4dor and three";

pub const INVITED_EMAIL: &str = "invited@example.com";

pub const AGENT_NAME: &str = "Tracker agent";

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
