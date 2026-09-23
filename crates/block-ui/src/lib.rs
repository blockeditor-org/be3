pub mod database;
pub mod datetime;

use std::collections::{BTreeMap, HashMap};

use block::BlockReference;
use block_client::{BlockHandleAccess, CachedBlock, presence::PresenceColor};
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

pub fn presence_color(color: PresenceColor) -> beui::Color32 {
    match color {
        PresenceColor::Red => beui::Color32::from_rgb(224, 82, 82),
        PresenceColor::Orange => beui::Color32::from_rgb(230, 140, 50),
        PresenceColor::Yellow => beui::Color32::from_rgb(214, 179, 41),
        PresenceColor::Green => beui::Color32::from_rgb(84, 171, 90),
        PresenceColor::Teal => beui::Color32::from_rgb(46, 173, 168),
        PresenceColor::Blue => beui::Color32::from_rgb(74, 134, 227),
        PresenceColor::Purple => beui::Color32::from_rgb(150, 100, 214),
        PresenceColor::Pink => beui::Color32::from_rgb(224, 104, 168),
    }
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
        name: Option<&block_client::properties::BlockName>,
    ) -> Self {
        let (name, automatic) = match name.filter(|name| !name.value.is_empty()) {
            Some(name) => (name.value.clone(), !name.manual),
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

    pub fn for_properties(
        types: &dyn BlockTypes,
        block_type: Uuid,
        properties: &BTreeMap<Uuid, Vec<u8>>,
    ) -> Self {
        Self::new(
            types,
            block_type,
            block_client::properties::read_name(properties).as_ref(),
        )
    }

    pub fn for_reference(types: &dyn BlockTypes, reference: &BlockReference) -> Self {
        Self::for_properties(types, reference.block_type, &reference.properties)
    }

    pub fn for_cached(types: &dyn BlockTypes, cached: &CachedBlock) -> Self {
        Self::for_properties(types, cached.block_type, &cached.properties)
    }

    pub fn for_handle(types: &dyn BlockTypes, handle: &dyn BlockHandleAccess) -> Self {
        Self::new(types, handle.block_type(), handle.block_name().as_ref())
    }
}
