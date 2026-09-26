pub mod database;
pub mod datetime;

use std::collections::HashMap;

use uuid::Uuid;

pub const EMBEDDED_EDITOR_PADDING: f32 = 12.0;
pub const EMBEDDED_EDITOR_TITLE_HEIGHT: f32 = 28.0;
pub const EMBEDDED_EDITOR_TITLE_GAP: f32 = 8.0;

pub fn embedded_editor_frame(width: f32, height: f32, scale: f32) -> (f32, f32) {
    (
        (width + EMBEDDED_EDITOR_PADDING * 2.0) * scale,
        (height
            + EMBEDDED_EDITOR_PADDING * 2.0
            + EMBEDDED_EDITOR_TITLE_HEIGHT
            + EMBEDDED_EDITOR_TITLE_GAP)
            * scale,
    )
}

pub trait BlockTypes {
    fn display_name(&self, block_type: Uuid) -> Option<&str>;
    fn icon(&self, block_type: Uuid) -> Option<&'static str>;
    fn child_edits(&self, _block_type: Uuid) -> ChildEdits {
        ChildEdits::default()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChildEdits {
    pub add: bool,
    pub delete: bool,
    pub replace: bool,
}

pub struct BlockTypeEntry {
    pub display_name: String,
    pub icon: Option<&'static str>,
    pub child_edits: ChildEdits,
}

#[derive(Default)]
pub struct BlockCatalog {
    types: HashMap<Uuid, BlockTypeEntry>,
}

impl BlockCatalog {
    pub fn new(types: impl IntoIterator<Item = (Uuid, BlockTypeEntry)>) -> Self {
        Self {
            types: types.into_iter().collect(),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Uuid, &BlockTypeEntry)> {
        self.types.iter()
    }
}

impl BlockTypes for BlockCatalog {
    fn display_name(&self, block_type: Uuid) -> Option<&str> {
        self.types
            .get(&block_type)
            .map(|entry| entry.display_name.as_str())
    }

    fn icon(&self, block_type: Uuid) -> Option<&'static str> {
        self.types.get(&block_type).and_then(|entry| entry.icon)
    }

    fn child_edits(&self, block_type: Uuid) -> ChildEdits {
        self.types
            .get(&block_type)
            .map_or_else(ChildEdits::default, |entry| entry.child_edits)
    }
}
#[derive(Clone, PartialEq)]
pub struct BlockLabel {
    pub block_type: Uuid,
    pub icon: Option<&'static str>,
    pub name: String,
    pub automatic: bool,
}

impl BlockLabel {
    pub fn new(
        types: &dyn BlockTypes,
        block_type: Uuid,
        name: Option<&str>,
        named_by_hand: bool,
    ) -> Self {
        let (name, automatic) = match name.filter(|name| !name.is_empty()) {
            Some(name) => (name.to_owned(), !named_by_hand),
            None => (
                types
                    .display_name(block_type)
                    .map(str::to_owned)
                    .unwrap_or_else(|| "Untitled".to_owned()),
                true,
            ),
        };
        Self {
            block_type,
            icon: types.icon(block_type),
            name,
            automatic,
        }
    }
}
