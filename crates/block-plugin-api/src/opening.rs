use crate::{EditorInstanceId, EditorMessage, OpenArtifact, OpenBlock, OpenCreation, Opening};

impl Opening {
    pub fn instance(&self) -> EditorInstanceId {
        match self {
            Self::Block(open) => open.instance,
            Self::Creation(open) => open.instance,
            Self::Artifact(open) => open.instance,
        }
    }

    pub fn of(message: EditorMessage) -> Result<Self, Box<EditorMessage>> {
        Ok(match message {
            EditorMessage::Open {
                instance,
                editor,
                block_id,
                block_type,
                view_block,
                account_id,
                organization_id,
                client_id,
                editable,
            } => Self::Block(OpenBlock {
                instance,
                editor,
                block_id,
                block_type,
                view_block,
                account_id,
                organization_id,
                client_id,
                editable,
            }),
            EditorMessage::OpenCreation {
                instance,
                editor,
                template,
                account_id,
                organization_id,
                client_id,
            } => Self::Creation(OpenCreation {
                instance,
                editor,
                template,
                account_id,
                organization_id,
                client_id,
            }),
            EditorMessage::OpenArtifact {
                instance,
                editor,
                block_id,
                block_type,
                account_id,
                organization_id,
                client_id,
                data,
            } => Self::Artifact(OpenArtifact {
                instance,
                editor,
                block_id,
                block_type,
                account_id,
                organization_id,
                client_id,
                data,
            }),
            message => return Err(Box::new(message)),
        })
    }
}

impl From<Opening> for EditorMessage {
    fn from(opening: Opening) -> Self {
        match opening {
            Opening::Block(OpenBlock {
                instance,
                editor,
                block_id,
                block_type,
                view_block,
                account_id,
                organization_id,
                client_id,
                editable,
            }) => Self::Open {
                instance,
                editor,
                block_id,
                block_type,
                view_block,
                account_id,
                organization_id,
                client_id,
                editable,
            },
            Opening::Creation(OpenCreation {
                instance,
                editor,
                template,
                account_id,
                organization_id,
                client_id,
            }) => Self::OpenCreation {
                instance,
                editor,
                template,
                account_id,
                organization_id,
                client_id,
            },
            Opening::Artifact(OpenArtifact {
                instance,
                editor,
                block_id,
                block_type,
                account_id,
                organization_id,
                client_id,
                data,
            }) => Self::OpenArtifact {
                instance,
                editor,
                block_id,
                block_type,
                account_id,
                organization_id,
                client_id,
                data,
            },
        }
    }
}
