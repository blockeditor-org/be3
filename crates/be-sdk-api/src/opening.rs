use crate::{EditorInstanceId, Opening};

impl Opening {
    pub fn instance(&self) -> EditorInstanceId {
        match self {
            Self::Block(open) => open.instance,
            Self::Creation(open) => open.instance,
            Self::Artifact(open) => open.instance,
            Self::Picker(open) => open.instance,
            Self::Rename(open) => open.instance,
        }
    }
}
