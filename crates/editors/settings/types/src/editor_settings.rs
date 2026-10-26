use be_model::{Document, Edit, Map, Model, ObjectId};
use uuid::Uuid;

use be_block::Root;
use be_block::setting::Setting;
use be_block::settings::Layered;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct EditorSettings {
    pub defaults: Map<Uuid, Uuid>,
}

impl EditorSettings {
    pub fn default_editor(&self, block_type: Uuid) -> Option<Uuid> {
        self.defaults.get(&block_type).copied()
    }

    pub fn set_default_editor(block_type: Uuid, editor: Option<Uuid>) -> Edit {
        Self::DEFAULTS
            .put(ObjectId::ROOT, &block_type, editor.as_ref())
            .into()
    }
}

impl EditorSettings {
    pub fn default_editor_setting(
        block_type: Uuid,
        name: impl Into<String>,
    ) -> Setting<Self, Uuid> {
        Setting::new(
            format!("default-editor.{block_type}"),
            name,
            move |settings: &Self| settings.default_editor(block_type),
            move |_, editor| Self::set_default_editor(block_type, editor),
        )
    }
}

impl Layered for EditorSettings {
    const NAME: &'static str = "Default editors";

    fn over(&self, lower: &Self) -> Self {
        Self {
            defaults: lower
                .defaults
                .iter()
                .chain(self.defaults.iter())
                .map(|(block_type, editor)| (*block_type, *editor))
                .collect(),
        }
    }
}

impl Root for EditorSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6564_6974_6f72_2d64_6566_6175_6c74_7331);
}

pub type EditorSettingsContent = Document<EditorSettings>;
