use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::{
    AppIdentity, AppManifest, BlockTypeDescriptor, BlockTypeManifest, ChildOperations,
    EditorCapabilities, EditorManifest, Edits, InteractionMode, ManifestError, ParentView, Press,
    ResizeMode, ShellRequest, TemplateCategory, TemplateManifest,
};

#[cfg(test)]
mod tests;

/// `manifest.json` as written: the app's single source of truth, read by the app for its
/// identity and by the host for everything else. Untrusted input: a bad one is skipped and
/// shown in the Apps debug window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestDocument {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub editors: InOrder<EditorDocument>,
    #[serde(default)]
    pub block_types: InOrder<BlockTypeDocument>,
    /// The app's wasm module, the same bytes on every platform; the BUCK file's `app()`
    /// names the module that builds it.
    pub entry_point: String,
    /// Host names `fetch` may reach, matched whole against an https URL's host.
    #[serde(default)]
    pub network: Vec<String>,
    /// Protocol packages beyond the core the app uses, of `be_sdk_api::EXTENSIONS`. The
    /// host denies calls into any other, and refuses a manifest naming one it does not know.
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorDocument {
    /// A fresh uuid for this editor. View blocks (`VIEW_EDITOR`), default-editor settings
    /// and templates name the editor by it, so changing it, or sharing it with another
    /// editor or a block type, breaks them.
    pub id: String,
    /// The `CONTENT_TYPE` of a type some manifest or the host declares, or `"any"` for every
    /// type. None makes a shell editor: opened only on its own view block, never offered by
    /// Open with or the add menu, and refused if it declares templates.
    #[serde(default)]
    pub edits: Option<String>,
    pub display_name: String,
    /// A codepoint of the Lucide icon font, not a name.
    pub icon: String,
    /// The add menu's entries, in the order written. An editor with none is never offered.
    #[serde(default)]
    pub templates: InOrder<TemplateDocument>,
    /// Whether an embedded instance runs live or shows a preview until focused.
    #[serde(default)]
    pub interaction: InteractionMode,
    #[serde(default)]
    pub capabilities: EditorCapabilities,
    #[serde(default)]
    pub resize: ResizeMode,
    /// The screens the host may give an instance: `Frame`, `Preview`, `ArtifactSettings`.
    pub views: Vec<ParentView>,
    /// For a shell editor: the requests it takes from editors inside it. The host hands a
    /// request to the nearest enclosing editor that accepts it, and its own requests to the
    /// outermost shell. `pick` and `rename` mark the editor the host runs picker or rename
    /// children in.
    #[serde(default)]
    pub accepts: Vec<ShellRequest>,
    /// Ids of editors a shell places as soon as it opens; the host starts their apps as
    /// early as the shell's.
    #[serde(default)]
    pub opens: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// A block type the app brings. It belongs to the type, not to any editor of it.
pub struct BlockTypeDocument {
    /// The type's `CONTENT_TYPE`.
    pub id: String,
    pub display_name: String,
    pub icon: String,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// Which structural edits from the file tree the type takes (`Root::child_edit`).
    #[serde(default)]
    pub children: ChildOperations,
    /// What the file tree does on a press: open the block, or `Zoom` into it (a folder).
    #[serde(default)]
    pub press: Press,
    /// The app's type module. Every type of an app names the same one, which the host runs
    /// once per app.
    pub module: String,
    /// Exporting a child of a block of this type exports the whole block (a database view,
    /// so its database never leaves without it).
    #[serde(default)]
    pub exported_whole: bool,
    /// The type has a plain-file form, so the file tree offers export before the module has
    /// run. A be-tool-cli test holds every manifest to its module.
    #[serde(default)]
    pub plain_file: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateDocument {
    /// Defaults to the editor's display name.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub category: CategoryDocument,
    /// The block cannot exist until the user chooses something, so the editor draws a
    /// creation dialog (`BeuiRoot::Creation`) before the block is made.
    #[serde(default)]
    pub dialog: bool,
    /// The type the template makes, if not the editor's (a slide template making a canvas).
    /// Required for an editor of `"any"` or of nothing.
    #[serde(default)]
    pub block_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Where the add menu offers a template: the main tab under Common, More blocks or Debug,
/// or the Templates tab, grouped under the declaring editor.
pub enum CategoryDocument {
    Important,
    #[default]
    Regular,
    Debug,
    Template,
}

impl From<CategoryDocument> for TemplateCategory {
    fn from(category: CategoryDocument) -> Self {
        match category {
            CategoryDocument::Important => Self::Important,
            CategoryDocument::Regular | CategoryDocument::Debug => Self::Regular,
            CategoryDocument::Template => Self::Template,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InOrder<T>(pub Vec<(String, T)>);

impl<T> Default for InOrder<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Serialize> Serialize for InOrder<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for InOrder<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries<T>(std::marker::PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for Entries<T> {
            type Value = InOrder<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map from name to entry")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<InOrder<T>, M::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, T>()? {
                    entries.push(entry);
                }
                Ok(InOrder(entries))
            }
        }

        deserializer.deserialize_map(Entries(std::marker::PhantomData))
    }
}

fn block_type(source: &str) -> Result<[u8; 16], ManifestError> {
    Uuid::parse_str(source)
        .map(Uuid::into_bytes)
        .map_err(|_| ManifestError::InvalidBlockType)
}

fn edits(source: Option<&str>) -> Result<Edits, ManifestError> {
    match source {
        None => Ok(Edits::Nothing),
        Some(Edits::ANY_NAME) => Ok(Edits::Any),
        Some(source) => block_type(source).map(Edits::Type),
    }
}

impl ManifestDocument {
    pub fn parse(source: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(source).map_err(|error| ManifestError::Malformed(error.to_string()))
    }

    pub fn identity(&self) -> AppIdentity {
        AppIdentity {
            id: self.id.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }

    pub fn into_manifest(self) -> Result<AppManifest, ManifestError> {
        let manifest = AppManifest {
            identity: AppIdentity {
                id: self.id,
                name: self.name,
                version: self.version,
            },
            editors: self
                .editors
                .0
                .into_iter()
                .map(|(name, editor)| editor.into_manifest(name))
                .collect::<Result<_, _>>()?,
            block_types: self
                .block_types
                .0
                .into_iter()
                .map(|(name, block_type)| block_type.into_manifest(name))
                .collect::<Result<_, _>>()?,
            entry_point: self.entry_point,
            network: self.network,
            extensions: self.extensions,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

impl BlockTypeDocument {
    fn into_manifest(self, name: String) -> Result<BlockTypeManifest, ManifestError> {
        Ok(BlockTypeManifest {
            name,
            descriptor: BlockTypeDescriptor {
                block_type: block_type(&self.id)?,
                display_name: self.display_name,
                color: self.color,
                icon_codepoint: self.icon,
                children: self.children,
                press: self.press,
                plain_file: self.plain_file,
            },
            module: self.module,
            exported_whole: self.exported_whole,
        })
    }
}

impl EditorDocument {
    fn into_manifest(self, name: String) -> Result<EditorManifest, ManifestError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|_| ManifestError::InvalidEditorId)?
            .into_bytes();
        let edits = edits(self.edits.as_deref())?;
        let templates = self
            .templates
            .0
            .into_iter()
            .map(|(id, template)| {
                Ok(TemplateManifest {
                    id,
                    name: template.name.unwrap_or_else(|| self.display_name.clone()),
                    icon: template.icon.unwrap_or_else(|| self.icon.clone()),
                    category: template.category.into(),
                    debug: template.category == CategoryDocument::Debug,
                    dialog: template.dialog,
                    block_type: match template.block_type {
                        Some(source) => block_type(&source)?,
                        None => edits
                            .block_type()
                            .ok_or(ManifestError::TemplateWithoutBlockType)?,
                    },
                })
            })
            .collect::<Result<_, ManifestError>>()?;
        Ok(EditorManifest {
            id,
            name,
            edits,
            display_name: self.display_name,
            icon: self.icon,
            templates,
            interaction: self.interaction,
            capabilities: self.capabilities,
            resize: self.resize,
            views: self.views,
            accepts: self.accepts,
            opens: self
                .opens
                .iter()
                .map(|editor| {
                    Uuid::parse_str(editor)
                        .map(Uuid::into_bytes)
                        .map_err(|_| ManifestError::InvalidEditorId)
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

pub fn manifest_from_json(source: &str) -> Result<AppManifest, ManifestError> {
    ManifestDocument::parse(source)?.into_manifest()
}
