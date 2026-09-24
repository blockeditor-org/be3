use be_model::{Anchor, Document, Edit, List, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;
use crate::blob::{Blob, BlobKind};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct GameModuleHeader {
    pub source_name: String,
}

pub struct GameModuleFile;

impl BlobKind for GameModuleFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6761_6d65_2d6d_6f64_756c_652d_6366_0002);

    type Header = GameModuleHeader;

    fn name(header: &GameModuleHeader) -> &str {
        &header.source_name
    }
}

pub type GameModuleContent = Blob<GameModuleFile>;

impl Blob<GameModuleFile> {
    pub fn from_file(source_name: impl Into<String>, data: Vec<u8>) -> Self {
        Self::new(
            GameModuleHeader {
                source_name: source_name.into(),
            },
            data,
        )
    }
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct DeterministicGame {
    pub module: Option<Uuid>,
    pub moves: List<GameMove>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct GameMove {
    pub actor: Uuid,
    pub action: Vec<u8>,
}

impl DeterministicGame {
    pub fn of(module: Uuid) -> Self {
        Self {
            module: Some(module),
            moves: List::default(),
        }
    }

    pub fn play(actor: Uuid, action: Vec<u8>) -> Edit {
        Self::MOVES
            .insert(ObjectId::ROOT, Anchor::End, &GameMove { actor, action })
            .1
            .into()
    }
}

impl Root for DeterministicGame {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6465_742d_6761_6d65_2d63_6f6e_7465_0002);

    fn references(&self) -> Vec<Uuid> {
        self.module.into_iter().collect()
    }
}

pub type DeterministicGameContent = Document<DeterministicGame>;
