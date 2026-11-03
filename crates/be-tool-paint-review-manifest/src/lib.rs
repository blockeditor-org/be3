use serde::{Deserialize, Serialize};

pub const IMAGE_EXTENSION: &str = "webp";

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
    pub changed_pixels: u64,
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
    pub before: Option<Picture>,
    pub after: Option<Picture>,
    pub changes: Option<Picture>,
    pub changed_pixels: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Picture {
    pub image: String,
    pub thumbnail: String,
    pub width: u32,
    pub height: u32,
}

impl Picture {
    pub fn area(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

impl Painting {
    pub fn sort(paintings: &mut [Painting]) {
        paintings.sort_by(|a, b| {
            b.changed_pixels
                .cmp(&a.changed_pixels)
                .then_with(|| a.status.cmp(&b.status))
                .then_with(|| a.name.cmp(&b.name))
        });
    }
}
