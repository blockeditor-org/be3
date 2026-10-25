use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::{
    BlockTypeDescriptor, BlockTypeManifest, ChildOperations, EditorCapabilities, EditorManifest,
    EditorRegion, Edits, InteractionMode, ManifestError, PluginIdentity, PluginManifest, Press,
    ResizeMode, ShellRequest, TemplateCategory, TemplateManifest,
};

#[cfg(test)]
mod tests;

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
    pub entry_point: String,
    #[serde(default)]
    pub network: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorDocument {
    pub id: String,
    #[serde(default)]
    pub edits: Option<String>,
    pub display_name: String,
    pub icon: String,
    #[serde(default)]
    pub templates: InOrder<TemplateDocument>,
    #[serde(default)]
    pub interaction: InteractionMode,
    #[serde(default)]
    pub capabilities: EditorCapabilities,
    #[serde(default)]
    pub resize: ResizeMode,
    pub regions: Vec<EditorRegion>,
    #[serde(default)]
    pub accepts: Vec<ShellRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockTypeDocument {
    pub id: String,
    pub display_name: String,
    pub icon: String,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub children: ChildOperations,
    #[serde(default)]
    pub press: Press,
    pub module: String,
    #[serde(default)]
    pub exported_whole: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateDocument {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub category: CategoryDocument,
    #[serde(default)]
    pub dialog: bool,
    #[serde(default)]
    pub block_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
            CategoryDocument::Regular => Self::Regular,
            CategoryDocument::Debug => Self::Debug,
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

    pub fn identity(&self) -> PluginIdentity {
        PluginIdentity {
            id: self.id.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }

    pub fn into_manifest(self) -> Result<PluginManifest, ManifestError> {
        let manifest = PluginManifest {
            identity: PluginIdentity {
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
                plain_file: false,
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
            regions: self.regions,
            accepts: self.accepts,
        })
    }
}

pub fn manifest_from_json(source: &str) -> Result<PluginManifest, ManifestError> {
    ManifestDocument::parse(source)?.into_manifest()
}
