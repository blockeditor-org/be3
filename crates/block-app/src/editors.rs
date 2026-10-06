pub(crate) mod plugin;

use std::collections::HashMap;
use std::sync::Arc;

use be_graph::Access;
use block_plugin_api::{EditorManifest, PluginManifest, TemplateCategory};
use uuid::Uuid;

pub(crate) use crate::block_label::BlockLabel;
use crate::surfaces::HostedRegion;

pub(crate) use self::plugin::PluginEditor;

pub struct FocusReport {
    pub block: Option<(Uuid, Uuid)>,
    pub via: Vec<Uuid>,
}

pub enum EditorAction {
    OpenBlock {
        id: Uuid,
        block_type: Uuid,
        via: Option<Uuid>,
    },
    DragBlock {
        id: Uuid,
        block_type: Uuid,
    },
    Command {
        id: Uuid,
        command: block_plugin_api::BlockCommand,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct DirectEditorCapabilities {
    pub allow_rotation: bool,
    pub preserve_aspect_ratio: bool,
    pub supports_pan_and_zoom: bool,
    pub max_zoom: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectEditorInteraction {
    Preview,
    Live,
    Playback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectEditorResize {
    None,
    Horizontal,
    Vertical,
    Both,
}

pub fn editor_access_ceiling(id: Uuid) -> Access {
    let Some(node) = crate::be::node(id) else {
        return Access::Edit;
    };
    match node.metadata.artifact.is_some() {
        true => node.access.min(Access::View),
        false => node.access,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarDragSource {
    Root,
    Orphaned,
    Block(Uuid),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Chrome {
    #[default]
    Drawn,
    None,
}

type OpenEditor = Box<dyn Fn(Uuid) -> PluginEditor>;
type CreateOptions = Box<dyn Fn() -> Box<dyn PendingCreation>>;

struct ArtifactProvider(Arc<PluginManifest>);

pub(super) trait ArtifactSession {
    fn poll(&mut self, registry: &EditorRegistry, data: &[u8]) -> ArtifactStatus;
    fn settings_region(&mut self, registry: &EditorRegistry) -> HostedRegion;
    fn take_draft(&mut self) -> Option<Vec<u8>>;
    fn settings_height(&self) -> f32;
    fn summary(&self, draft: &[u8]) -> Option<String>;
    fn cancel_settings(&mut self);
    fn regenerate(&mut self, data: &[u8]);
    fn take_outcome(&mut self) -> Option<Result<(), String>>;
    fn regenerating(&self) -> bool;
}

pub(super) enum ArtifactStatus {
    Starting,
    Described { source: Uuid, summary: String },
    Failed(String),
}

pub(crate) trait PendingCreation {
    fn region(&self, registry: &EditorRegistry, client_id: Uuid) -> Option<HostedRegion>;
    fn pick_source(&self) -> plugin::PickSource;
    fn step(&mut self, registry: &EditorRegistry, client_id: Uuid) -> CreationStep;
    fn height(&self) -> Option<f32>;
    fn create(&mut self) -> Result<Option<Uuid>, String>;
}

#[derive(Clone, Copy)]
pub(crate) enum CreationStep {
    Options(bool),
    Working,
}

struct EditorRegistration {
    block_type: Uuid,
    display_name: &'static str,
    icon: &'static str,
    open: OpenEditor,
    can_add_child: bool,
    can_delete_child: bool,
    can_replace_child: bool,
    dynamic_artifact: Option<ArtifactProvider>,
}

pub(crate) struct TemplateEntry {
    pub(crate) target: plugin::CreationTarget,
    pub(crate) icon: &'static str,
    pub(crate) category: TemplateCategory,
    create: CreateOptions,
}

pub(crate) struct BlockTypeEntry {
    pub(crate) display_name: String,
    pub(crate) icon: &'static str,
    pub(crate) add: bool,
    pub(crate) delete: bool,
    pub(crate) replace: bool,
}

pub struct EditorRegistry {
    registrations: HashMap<Uuid, EditorRegistration>,
    templates: Vec<TemplateEntry>,
    plugin_block_types: Arc<block_plugin_api::Catalog>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        Self::from_manifests(plugin::discovery::manifests())
    }

    pub(crate) fn from_manifests(manifests: Vec<Arc<PluginManifest>>) -> Self {
        let mut registry = Self {
            registrations: HashMap::new(),
            templates: Vec::new(),
            plugin_block_types: Arc::default(),
        };
        for manifest in manifests {
            registry.register_plugin(manifest);
        }
        registry.plugin_block_types = Arc::new(block_plugin_api::Catalog {
            types: plugin::block_type_descriptors(registry.block_types()),
            templates: registry
                .templates
                .iter()
                .map(|entry| block_plugin_api::TemplateDescriptor {
                    editor: entry.target.editor.into_bytes(),
                    template: entry.target.template.to_owned(),
                    block_type: entry.target.block_type.into_bytes(),
                    name: entry.target.name.to_owned(),
                    icon_codepoint: entry.icon.to_owned(),
                    category: entry.category,
                    dialog: entry.target.dialog,
                })
                .collect(),
        });
        registry
    }

    fn block_types(&self) -> Vec<(Uuid, BlockTypeEntry)> {
        let mut types: Vec<_> = self
            .registrations
            .values()
            .map(|registration| {
                (
                    registration.block_type,
                    BlockTypeEntry {
                        display_name: registration.display_name.to_owned(),
                        icon: registration.icon,
                        add: registration.can_add_child,
                        delete: registration.can_delete_child,
                        replace: registration.can_replace_child,
                    },
                )
            })
            .collect();
        types.sort_by_key(|(block_type, _)| *block_type);
        types
    }

    pub(super) fn plugin_block_types(&self) -> &Arc<block_plugin_api::Catalog> {
        &self.plugin_block_types
    }

    fn register_plugin(&mut self, manifest: Arc<PluginManifest>) {
        for editor in &manifest.editors {
            self.register_editor(&manifest, editor);
        }
    }

    fn register_editor(&mut self, manifest: &Arc<PluginManifest>, editor: &EditorManifest) {
        let block_type = Uuid::from_bytes(editor.block_type);
        let display_name: &'static str = Box::leak(editor.display_name.clone().into_boxed_str());
        let icon: &'static str = Box::leak(editor.icon.clone().into_boxed_str());
        for template in &editor.templates {
            let target = plugin::CreationTarget {
                editor: block_type,
                template: Box::leak(template.id.clone().into_boxed_str()),
                block_type: Uuid::from_bytes(template.block_type),
                dialog: template.dialog,
                name: Box::leak(template.name.clone().into_boxed_str()),
            };
            let plugin = Arc::clone(manifest);
            self.templates.push(TemplateEntry {
                target,
                icon: Box::leak(template.icon.clone().into_boxed_str()),
                category: template.category,
                create: Box::new(move || {
                    Box::new(plugin::PluginCreation::new(Arc::clone(&plugin), target))
                }),
            });
        }
        self.registrations.insert(
            block_type,
            EditorRegistration {
                block_type,
                display_name,
                icon,
                open: {
                    let manifest = Arc::clone(manifest);
                    Box::new(move |id| PluginEditor::new(Arc::clone(&manifest), id, block_type))
                },
                can_add_child: editor.children.add,
                can_delete_child: editor.children.delete,
                can_replace_child: editor.children.replace,
                dynamic_artifact: editor
                    .regions
                    .contains(&block_plugin_api::EditorRegion::ArtifactSettings)
                    .then(|| ArtifactProvider(Arc::clone(manifest))),
            },
        );
    }

    pub fn display_name(&self, block_type: Uuid) -> Option<&'static str> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.display_name)
    }

    pub fn icon(&self, block_type: Uuid) -> Option<&'static str> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.icon)
    }

    pub(super) fn artifact_session(
        &self,
        source_type: Uuid,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Result<Box<dyn ArtifactSession>, String> {
        let registration = self
            .registrations
            .get(&source_type)
            .ok_or_else(|| format!("unsupported dynamic artifact source type {source_type}"))?;
        match &registration.dynamic_artifact {
            Some(ArtifactProvider(manifest)) => Ok(Box::new(plugin::PluginArtifact::new(
                Arc::clone(manifest),
                source_type,
                target_id,
                target_type,
                client_id,
            ))),
            None => Err(format!(
                "{} blocks do not generate dynamic artifacts",
                registration.display_name
            )),
        }
    }

    pub(crate) fn create(
        &self,
        editor: Uuid,
        template: &str,
    ) -> Option<(plugin::CreationTarget, Box<dyn PendingCreation>)> {
        let entry = self
            .templates
            .iter()
            .find(|entry| entry.target.editor == editor && entry.target.template == template)?;
        Some((entry.target, (entry.create)()))
    }

    pub fn open(&self, id: Uuid, block_type: Uuid) -> PluginEditor {
        self.registrations.get(&block_type).map_or_else(
            || PluginEditor::unsupported(id, block_type),
            |registration| (registration.open)(id),
        )
    }
}
