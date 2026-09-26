use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::{
    ChildOperations, EditorCapabilities, EditorManifest, EditorRegion, InteractionMode,
    ManifestError, PluginIdentity, PluginManifest, ResizeMode, TemplateCategory, TemplateManifest,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestDocument {
    pub id: String,
    pub name: String,
    pub version: String,
    pub editors: Vec<EditorDocument>,
    pub entry_point: String,
    #[serde(default)]
    pub network: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorDocument {
    pub block_type: String,
    pub display_name: String,
    pub icon: String,
    #[serde(default)]
    pub templates: Templates,
    #[serde(default)]
    pub children: ChildOperations,
    #[serde(default)]
    pub interaction: InteractionMode,
    #[serde(default)]
    pub capabilities: EditorCapabilities,
    #[serde(default)]
    pub resize: ResizeMode,
    pub regions: Vec<EditorRegion>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateDocument {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub category: TemplateCategory,
    #[serde(default)]
    pub dialog: bool,
    #[serde(default)]
    pub block_type: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Templates(pub Vec<(String, TemplateDocument)>);

impl Serialize for Templates {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (id, template) in &self.0 {
            map.serialize_entry(id, template)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Templates {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct InOrder;

        impl<'de> Visitor<'de> for InOrder {
            type Value = Templates;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map from template id to template")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Templates, M::Error> {
                let mut templates = Vec::new();
                while let Some(entry) = map.next_entry::<String, TemplateDocument>()? {
                    templates.push(entry);
                }
                Ok(Templates(templates))
            }
        }

        deserializer.deserialize_map(InOrder)
    }
}

fn block_type(source: &str) -> Result<[u8; 16], ManifestError> {
    Uuid::parse_str(source)
        .map(Uuid::into_bytes)
        .map_err(|_| ManifestError::InvalidBlockType)
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
                .into_iter()
                .map(EditorDocument::into_manifest)
                .collect::<Result<_, _>>()?,
            entry_point: self.entry_point,
            network: self.network,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

impl EditorDocument {
    fn into_manifest(self) -> Result<EditorManifest, ManifestError> {
        let editor_type = block_type(&self.block_type)?;
        let templates = self
            .templates
            .0
            .into_iter()
            .map(|(id, template)| {
                Ok(TemplateManifest {
                    id,
                    name: template.name.unwrap_or_else(|| self.display_name.clone()),
                    icon: template.icon.unwrap_or_else(|| self.icon.clone()),
                    category: template.category,
                    dialog: template.dialog,
                    block_type: match template.block_type {
                        Some(source) => block_type(&source)?,
                        None => editor_type,
                    },
                })
            })
            .collect::<Result<_, ManifestError>>()?;
        Ok(EditorManifest {
            block_type: editor_type,
            display_name: self.display_name,
            icon: self.icon,
            templates,
            children: self.children,
            interaction: self.interaction,
            capabilities: self.capabilities,
            resize: self.resize,
            regions: self.regions,
        })
    }
}

pub fn manifest_from_json(source: &str) -> Result<PluginManifest, ManifestError> {
    ManifestDocument::parse(source)?.into_manifest()
}
