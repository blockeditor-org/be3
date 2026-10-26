pub(crate) mod plugin;

use std::{collections::HashMap, sync::Arc};

use be_graph::Access;
use block_plugin_api::{EditorManifest, Edits, PluginManifest, TemplateCategory};
use uuid::Uuid;

use crate::surfaces::HostedRegion;

pub(crate) use self::plugin::PluginEditor;

pub struct FocusReport {
    pub block: Option<(Uuid, Uuid)>,
    pub via: Vec<Uuid>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockOpen {
    pub id: Uuid,
    pub block_type: Uuid,
    pub via: Option<Uuid>,
    pub editor: Option<Uuid>,
}

pub enum EditorAction {
    OpenBlock(BlockOpen),
    Command {
        id: Uuid,
        command: block_plugin_api::BlockCommand,
    },
    Shell {
        id: Uuid,
        command: block_plugin_api::ShellCommand,
    },
    SimulateAccess {
        id: Uuid,
        access: block_plugin_api::AccessLevel,
    },
    AppMenu {
        at: Option<beui::Rect>,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct DirectEditorCapabilities {
    pub allow_rotation: bool,
    pub preserve_aspect_ratio: bool,
    pub supports_pan_and_zoom: bool,
    pub max_zoom: Option<u32>,
    pub pan: block_plugin_api::PanMode,
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
        return Access::Manage;
    };
    match node.metadata.contains(&be_block::ARTIFACT) {
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

type CreateOptions = Box<dyn Fn() -> Box<dyn PendingCreation>>;

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
    Described(String),
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

pub(crate) struct TemplateEntry {
    pub(crate) target: plugin::CreationTarget,
    pub(crate) icon: &'static str,
    pub(crate) category: TemplateCategory,
    pub(crate) debug: bool,
    create: CreateOptions,
}

#[derive(Clone, Copy)]
pub(crate) struct Opened {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) view_block: Option<Uuid>,
}

struct RegisteredEditor {
    id: Uuid,
    edits: Edits,
    manifest: Arc<PluginManifest>,
    name: String,
    icon: String,
}

impl RegisteredEditor {
    fn edits(&self, opened: Opened) -> bool {
        match self.edits {
            Edits::Type(edited) => Uuid::from_bytes(edited) == opened.block_type,
            Edits::Any => true,
            Edits::Nothing => opened.view_block == Some(opened.id),
        }
    }

    fn open(&self, id: Uuid, block_type: Uuid) -> PluginEditor {
        PluginEditor::new(Arc::clone(&self.manifest), self.id, id, block_type)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PluginCatalog {
    pub(crate) catalog: block_plugin_api::Catalog,
    pub(crate) debug_templates: Vec<block_plugin_api::DebugTemplate>,
}

pub struct EditorRegistry {
    editors: Vec<RegisteredEditor>,
    templates: Vec<TemplateEntry>,
    types: HashMap<Uuid, block_plugin_api::BlockTypeDescriptor>,
    plugin_block_types: Arc<crate::editors::PluginCatalog>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        Self::from_manifests(plugin::discovery::manifests())
    }

    pub(crate) fn from_manifests(manifests: Vec<Arc<PluginManifest>>) -> Self {
        let mut registry = Self {
            editors: Vec::new(),
            templates: Vec::new(),
            types: plugin::discovery::block_types(&manifests),
            plugin_block_types: Arc::default(),
        };
        for manifest in manifests {
            for editor in &manifest.editors {
                registry.register_editor(&manifest, editor);
            }
        }
        let debug_templates = registry
            .templates
            .iter()
            .filter(|entry| entry.debug)
            .map(|entry| block_plugin_api::DebugTemplate {
                editor: entry.target.editor.into_bytes(),
                template: entry.target.template.to_owned(),
            })
            .collect();
        let catalog = block_plugin_api::Catalog {
            types: {
                let mut types: Vec<_> = registry.types.values().cloned().collect();
                types.sort_by_key(|descriptor| descriptor.block_type);
                types
            },
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
            editors: registry
                .editors
                .iter()
                .map(|editor| block_plugin_api::EditorDescriptor {
                    editor: editor.id.into_bytes(),
                    edits: editor.edits,
                    name: editor.name.clone(),
                    icon_codepoint: editor.icon.clone(),
                })
                .collect(),
        };
        registry.plugin_block_types = Arc::new(PluginCatalog {
            catalog,
            debug_templates,
        });
        registry
    }

    pub(super) fn plugin_block_types(&self) -> &Arc<crate::editors::PluginCatalog> {
        &self.plugin_block_types
    }

    fn register_editor(&mut self, manifest: &Arc<PluginManifest>, editor: &EditorManifest) {
        let id = Uuid::from_bytes(editor.id);
        self.editors.push(RegisteredEditor {
            id,
            edits: editor.edits,
            manifest: Arc::clone(manifest),
            name: editor.display_name.clone(),
            icon: editor.icon.clone(),
        });
        for template in &editor.templates {
            let target = plugin::CreationTarget {
                editor: id,
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
                debug: template.debug,
                create: Box::new(move || {
                    Box::new(plugin::PluginCreation::new(Arc::clone(&plugin), target))
                }),
            });
        }
    }

    pub fn display_name(&self, block_type: Uuid) -> Option<&str> {
        self.types
            .get(&block_type)
            .map(|block_type| block_type.display_name.as_str())
    }

    fn default_editor(&self, block_type: Uuid) -> Option<&RegisteredEditor> {
        self.editors
            .iter()
            .find(|editor| editor.edits == Edits::Type(block_type.into_bytes()))
    }

    pub(super) fn artifact_session(
        &self,
        source_type: Uuid,
        source: Uuid,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Result<Box<dyn ArtifactSession>, String> {
        let editor = self
            .default_editor(source_type)
            .ok_or_else(|| format!("unsupported dynamic artifact source type {source_type}"))?;
        let generates = editor
            .manifest
            .editor(editor.id.into_bytes())
            .is_some_and(|manifest| {
                manifest
                    .regions
                    .contains(&block_plugin_api::EditorRegion::ArtifactSettings)
            });
        match generates {
            true => Ok(Box::new(plugin::PluginArtifact::new(
                Arc::clone(&editor.manifest),
                editor.id,
                source,
                target_id,
                target_type,
                client_id,
            ))),
            false => Err(format!(
                "{} blocks do not generate dynamic artifacts",
                self.display_name(source_type).unwrap_or("These")
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
        self.default_editor(block_type).map_or_else(
            || PluginEditor::unsupported(id, block_type),
            |editor| editor.open(id, block_type),
        )
    }

    fn choice(&self, editor: Uuid, opened: Opened) -> Option<&RegisteredEditor> {
        self.editors
            .iter()
            .find(|registered| registered.id == editor && registered.edits(opened))
    }

    pub(crate) fn edits(&self, editor: Uuid, opened: Opened) -> bool {
        self.choice(editor, opened).is_some()
    }

    pub(crate) fn open_with(&self, opened: Opened, editor: Option<Uuid>) -> PluginEditor {
        let Opened {
            id,
            block_type,
            view_block,
        } = opened;
        match editor.and_then(|editor| self.choice(editor, opened)) {
            Some(choice) => choice.open(id, block_type),
            None => self.open(id, block_type),
        }
        .viewed_by(view_block)
    }
}
