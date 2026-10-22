use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
    pub mime_types: Vec<String>,
}

impl FileFilter {
    pub fn new(name: &str, extensions: &[&str], mime_types: &[&str]) -> Self {
        let owned = |values: &[&str]| values.iter().map(|value| (*value).to_owned()).collect();
        Self {
            name: name.to_owned(),
            extensions: owned(extensions),
            mime_types: owned(mime_types),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickedFile {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

pub type FilePick = Result<Option<PickedFile>, String>;
