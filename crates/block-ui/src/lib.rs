use lucide::Glyph;
pub mod database;
pub mod datetime;
#[cfg(test)]
mod tests;

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
    fn icon(&self, block_type: Uuid) -> Option<Glyph>;
    fn color(&self, _block_type: Uuid) -> Option<[u8; 3]> {
        None
    }
    fn child_edits(&self, _block_type: Uuid) -> ChildEdits {
        ChildEdits::default()
    }
    fn press(&self, _block_type: Uuid) -> Press {
        Press::Open
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Press {
    #[default]
    Open,
    Zoom,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChildEdits {
    pub add: bool,
    pub delete: bool,
    pub replace: bool,
}

pub struct BlockTypeEntry {
    pub display_name: String,
    pub color: Option<[u8; 3]>,
    pub icon: Option<Glyph>,
    pub child_edits: ChildEdits,
    pub press: Press,
    pub plain_file: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum TemplateCategory {
    Important,
    #[default]
    Regular,
    Debug,
    Template,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateEntry {
    pub color: Option<[u8; 3]>,
    pub editor: Uuid,
    pub template: String,
    pub block_type: Uuid,
    pub name: String,
    pub icon: Option<Glyph>,
    pub category: TemplateCategory,
    pub dialog: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorEntry {
    pub editor: Uuid,
    pub edits: Edits,
    pub name: String,
    pub icon: Option<Glyph>,
    pub extensions: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edits {
    Type(Uuid),
    Any,
    Nothing,
}

impl EditorEntry {
    fn edits_type(&self, block_type: Uuid) -> bool {
        match self.edits {
            Edits::Type(edited) => edited == block_type,
            Edits::Any => true,
            Edits::Nothing => false,
        }
    }

    fn opens_file(&self, file_name: Option<&str>) -> bool {
        let Some((stem, extension)) = file_name.and_then(|name| name.rsplit_once('.')) else {
            return false;
        };
        !stem.is_empty()
            && self
                .extensions
                .iter()
                .any(|listed| listed.eq_ignore_ascii_case(extension))
    }
}

#[derive(Default)]
pub struct BlockCatalog {
    types: HashMap<Uuid, BlockTypeEntry>,
    templates: Vec<TemplateEntry>,
    editors: Vec<EditorEntry>,
}

impl BlockCatalog {
    pub fn new(types: impl IntoIterator<Item = (Uuid, BlockTypeEntry)>) -> Self {
        Self {
            types: types.into_iter().collect(),
            templates: Vec::new(),
            editors: Vec::new(),
        }
    }

    pub fn with_editors(mut self, editors: impl IntoIterator<Item = EditorEntry>) -> Self {
        self.editors = editors.into_iter().collect();
        self
    }

    pub fn editors_for(&self, block_type: Uuid, file_name: Option<&str>) -> Vec<&EditorEntry> {
        let plain = self
            .editors
            .iter()
            .filter(|editor| editor.extensions.is_empty());
        let specific = plain
            .clone()
            .filter(|editor| editor.edits == Edits::Type(block_type));
        let programs = self.programs_for(block_type, file_name);
        let any = plain.filter(|editor| editor.edits == Edits::Any);
        specific.chain(programs).chain(any).collect()
    }

    fn programs_for(
        &self,
        block_type: Uuid,
        file_name: Option<&str>,
    ) -> impl Iterator<Item = &EditorEntry> + Clone {
        let plain_file = self.plain_file(block_type);
        self.editors.iter().filter(move |editor| {
            !editor.extensions.is_empty()
                && plain_file
                && editor.edits_type(block_type)
                && editor.opens_file(file_name)
        })
    }

    pub fn edits(&self, editor: Uuid, block_type: Uuid, file_name: Option<&str>) -> bool {
        self.editors_for(block_type, file_name)
            .iter()
            .any(|entry| entry.editor == editor)
    }

    pub fn plain_file(&self, block_type: Uuid) -> bool {
        self.types
            .get(&block_type)
            .is_some_and(|entry| entry.plain_file)
    }

    pub fn editor(&self, editor: Uuid) -> Option<&EditorEntry> {
        self.editors.iter().find(|entry| entry.editor == editor)
    }

    pub fn own_editor(&self, block_type: Uuid, file_name: Option<&str>) -> Option<Uuid> {
        self.editors
            .iter()
            .find(|editor| editor.extensions.is_empty() && editor.edits == Edits::Type(block_type))
            .or_else(|| self.programs_for(block_type, file_name).next())
            .map(|editor| editor.editor)
    }

    pub fn with_templates(mut self, templates: impl IntoIterator<Item = TemplateEntry>) -> Self {
        self.templates = templates.into_iter().collect();
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Uuid, &BlockTypeEntry)> {
        self.types.iter()
    }

    pub fn templates(&self) -> &[TemplateEntry] {
        &self.templates
    }
}

impl BlockTypes for BlockCatalog {
    fn display_name(&self, block_type: Uuid) -> Option<&str> {
        self.types
            .get(&block_type)
            .map(|entry| entry.display_name.as_str())
    }

    fn icon(&self, block_type: Uuid) -> Option<Glyph> {
        self.types.get(&block_type).and_then(|entry| entry.icon)
    }

    fn color(&self, block_type: Uuid) -> Option<[u8; 3]> {
        self.types.get(&block_type).and_then(|entry| entry.color)
    }

    fn child_edits(&self, block_type: Uuid) -> ChildEdits {
        self.types
            .get(&block_type)
            .map_or_else(ChildEdits::default, |entry| entry.child_edits)
    }

    fn press(&self, block_type: Uuid) -> Press {
        self.types
            .get(&block_type)
            .map_or(Press::Open, |entry| entry.press)
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Reach {
    #[default]
    Open,
    Locked,
    NoAccess,
}

impl Reach {
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Open => None,
            Self::Locked => Some("Locked"),
            Self::NoAccess => Some("No access"),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct BlockLabel {
    pub block_type: Uuid,
    pub icon: Option<Glyph>,
    pub name: String,
    pub automatic: bool,
    pub reach: Reach,
    pub deleted: bool,
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
            reach: Reach::Open,
            deleted: false,
        }
    }

    pub fn reached(mut self, reach: Reach) -> Self {
        if let Some(label) = reach.label() {
            label.clone_into(&mut self.name);
            self.automatic = true;
            self.icon = None;
        }
        self.reach = reach;
        self
    }

    pub fn deleted(mut self, deleted: bool) -> Self {
        self.deleted = deleted;
        self
    }
}
