use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub paintings: Vec<Painting>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Painting {
    pub name: String,
    pub status: Status,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub frames: Vec<Frame>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Changed,
    Added,
    Removed,
    Unreadable,
}

impl Status {
    pub const ALL: [Status; 4] = [
        Status::Changed,
        Status::Added,
        Status::Removed,
        Status::Unreadable,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub number: usize,
    pub before: Option<String>,
    pub after: Option<String>,
    pub changes: Option<String>,
    pub changed_pixels: Option<u64>,
}
