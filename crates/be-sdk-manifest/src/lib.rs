use serde::{Deserialize, Serialize};
use std::fmt;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "manifest",
    deps: ["../be-wasm-ops/wit"],
    with: {"uuid": [u8; 16], "rgb": [u8; 3]},
    copy: ["uuid", "rgb"],
    default: [
        "interaction-mode", "pan-mode", "editor-capabilities", "resize-mode", "child-operations",
        "press", "template-category",
    ],
    lenient: ["editor-capabilities", "child-operations"],
});

mod document;

pub use document::{
    EditorDocument, InOrder, ManifestDocument, TemplateDocument, manifest_from_json,
};
pub use types::{
    AppIdentity, BlockTypeDescriptor, ChildOperations, EditorCapabilities, Edits, InteractionMode,
    PanMode, ParentView, Press, ResizeMode, TemplateCategory,
};

pub const MAX_STRING_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppManifest {
    pub identity: AppIdentity,
    pub editors: Vec<EditorManifest>,
    pub block_types: Vec<BlockTypeManifest>,
    pub entry_point: String,
    pub network: Vec<String>,
    pub extensions: Vec<String>,
    pub block_agents: bool,
    /// Whether the app may grant or reveal access (guides/apps/host_services.md).
    pub sharing: bool,
    /// The app's host components (guides/apps/host_services.md, "Host components").
    pub hosts: Vec<HostManifest>,
    /// The app's worker modules, by file name, which it may start through the sandbox API
    /// (guides/adding_an_app.md, Workers).
    pub workers: Vec<String>,
    /// Whether the app is a root guest (plans/host_as_platform_adapter.md, The root guest),
    /// which `be-host --root=APP` runs in place of its own UI: a module with no editors, block
    /// types or host components of its own, staged as an app is so it spawns the apps it embeds.
    pub root: bool,
}

/// A host component: a component of the app's wasm with no frame, which the host opens on a
/// block of the type it `edits`, attached to the replica as an editor with Edit is, with no
/// view and no input. It is how a hosted block's host runs (plans/hosted_blocks.md), in
/// be-host or headless in `be` (plans/headless_apps.md).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostManifest {
    pub id: [u8; 16],
    pub name: String,
    /// The block type it runs on.
    pub edits: [u8; 16],
    /// The levels a block of the type is shared at, each named and said in a line for what
    /// it adds for the type's stored content (plans/hosted_blocks.md, "The share dialog"):
    /// Join always, in the order the share dialog lists them.
    pub levels: Vec<OfferedLevel>,
}

/// A level a hosted type offers in the share dialog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferedLevel {
    pub access: OfferedAccess,
    /// What the dialog calls it ("Join game", "Host").
    pub name: String,
    /// One line saying what it lets someone do with the stored block.
    pub line: String,
}

/// The core levels a hosted type may offer: every one a grant may give.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferedAccess {
    Join,
    View,
    Edit,
    Manage,
}

pub const EXTENSIONS: &[&str] = &["be3:desktop"];

/// The `network` entry that reaches every host, over https only like any other.
pub const ANY_HOST: &str = "*";

pub fn extension_of(interface: &str) -> Option<&'static str> {
    let package = interface.split_once('/')?.0;
    EXTENSIONS
        .iter()
        .copied()
        .find(|extension| *extension == package)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTypeManifest {
    pub name: String,
    pub descriptor: BlockTypeDescriptor,
    pub module: String,
    pub exported_whole: bool,
    /// The type counts what is unread (`counts_unread` in the manifest).
    pub counts_unread: bool,
}

impl BlockTypeManifest {
    pub fn content_type(&self) -> [u8; 16] {
        self.descriptor.block_type
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorManifest {
    pub id: [u8; 16],
    pub name: String,
    pub edits: Edits,
    pub display_name: String,
    pub icon: String,
    pub templates: Vec<TemplateManifest>,
    pub interaction: InteractionMode,
    pub capabilities: EditorCapabilities,
    pub resize: ResizeMode,
    pub views: Vec<ParentView>,
    pub accepts: Vec<ShellRequest>,
    pub opens: Vec<[u8; 16]>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellRequest {
    ShowBlock,
    Pick,
    Rename,
    Dialog,
    Panel,
    Window,
}

impl ShellRequest {
    pub const ROUTED: [Self; 4] = [Self::ShowBlock, Self::Dialog, Self::Panel, Self::Window];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateManifest {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub category: TemplateCategory,
    pub debug: bool,
    pub dialog: bool,
    pub block_type: [u8; 16],
}

impl AppManifest {
    pub fn grants(&self, extension: &str) -> bool {
        self.extensions.iter().any(|granted| granted == extension)
    }

    pub fn editor(&self, id: [u8; 16]) -> Option<&EditorManifest> {
        self.editors.iter().find(|editor| editor.id == id)
    }

    pub fn type_module(&self) -> Option<&str> {
        self.block_types
            .first()
            .map(|block_type| block_type.module.as_str())
    }
}

impl Edits {
    pub const ANY_NAME: &str = "any";

    pub fn block_type(self) -> Option<[u8; 16]> {
        match self {
            Self::Type(block_type) => Some(block_type),
            Self::Any | Self::Nothing => None,
        }
    }
}

impl EditorManifest {
    pub fn template(&self, id: &str) -> Option<&TemplateManifest> {
        self.templates.iter().find(|template| template.id == id)
    }

    pub fn accepts(&self, request: ShellRequest) -> bool {
        self.accepts.contains(&request)
    }
}

impl ResizeMode {
    pub fn horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    pub fn vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

impl ParentView {
    pub const ALL: [Self; 3] = [Self::Main, Self::Preview, Self::ArtifactSettings];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    Malformed(String),
    Empty(&'static str),
    TooLong(&'static str),
    InvalidIdentity,
    InvalidBlockType,
    InvalidEditorId,
    InvalidViews,
    InvalidNetworkHost,
    UnknownExtension,
    NoEditors,
    RootWithComponents,
    DuplicateEditor,
    DuplicateBlockType,
    SeveralTypeModules,
    DuplicateTemplate,
    TemplateWithoutBlockType,
    ServiceWithTemplates,
    DuplicateHost,
    InvalidLevels,
    InvalidWorker,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => write!(formatter, "the manifest could not be read: {error}"),
            Self::Empty(field) => write!(formatter, "{field} is empty"),
            Self::TooLong(field) => write!(formatter, "{field} is too long"),
            Self::InvalidIdentity => formatter.write_str("the app id is not a plain identifier"),
            Self::InvalidBlockType => formatter.write_str("the block type is not a uuid"),
            Self::InvalidEditorId => formatter.write_str("the editor id is not a uuid"),
            Self::InvalidViews => {
                formatter.write_str("the views must include the main view exactly once")
            }
            Self::InvalidNetworkHost => {
                formatter.write_str("a network host is not a plain host name")
            }
            Self::UnknownExtension => {
                formatter.write_str("an extension is not one the host knows, or is named twice")
            }
            Self::NoEditors => {
                formatter.write_str("the app declares no editors and no block types")
            }
            Self::RootWithComponents => formatter
                .write_str("a root declares no editors, block types or host components"),
            Self::InvalidWorker => formatter.write_str(
                "a worker is not a plain .wasm file name, is named twice, or is the entry point",
            ),
            Self::DuplicateHost => formatter.write_str(
                "a host component shares an id or name with an editor or another host, or two run on one type",
            ),
            Self::InvalidLevels => formatter.write_str(
                "a host component's levels must offer Join, and each level once",
            ),
            Self::DuplicateBlockType => {
                formatter.write_str("two block types are declared with the same id or name")
            }
            Self::SeveralTypeModules => {
                formatter.write_str("the block types name more than one type module")
            }
            Self::DuplicateEditor => {
                formatter.write_str("two editors are declared with the same id or name")
            }
            Self::TemplateWithoutBlockType => formatter.write_str(
                "a template of an editor that edits no one block type names no block type",
            ),
            Self::DuplicateTemplate => {
                formatter.write_str("an editor declares the same template twice")
            }
            Self::ServiceWithTemplates => formatter.write_str(
                "an editor that edits no block is part of the shell and declares no templates",
            ),
        }
    }
}

impl AppManifest {
    pub fn validate(&self) -> Result<(), ManifestError> {
        manifest_string("app id", &self.identity.id)?;
        manifest_string("app name", &self.identity.name)?;
        manifest_string("app version", &self.identity.version)?;
        if !self
            .identity
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(ManifestError::InvalidIdentity);
        }
        if self.root {
            if !self.editors.is_empty() || !self.block_types.is_empty() || !self.hosts.is_empty() {
                return Err(ManifestError::RootWithComponents);
            }
        } else if self.editors.is_empty() && self.block_types.is_empty() {
            return Err(ManifestError::NoEditors);
        }
        for (index, block_type) in self.block_types.iter().enumerate() {
            if self.block_types[..index].iter().any(|other| {
                other.content_type() == block_type.content_type() || other.name == block_type.name
            }) {
                return Err(ManifestError::DuplicateBlockType);
            }
            manifest_string("block type name", &block_type.name)?;
            manifest_string("display name", &block_type.descriptor.display_name)?;
            manifest_string("icon", &block_type.descriptor.icon_codepoint)?;
            manifest_string("type module", &block_type.module)?;
            if self.type_module() != Some(block_type.module.as_str()) {
                return Err(ManifestError::SeveralTypeModules);
            }
        }
        for (index, editor) in self.editors.iter().enumerate() {
            if self.editors[..index]
                .iter()
                .any(|other| other.id == editor.id || other.name == editor.name)
            {
                return Err(ManifestError::DuplicateEditor);
            }
            editor.validate()?;
        }
        for (index, host) in self.hosts.iter().enumerate() {
            manifest_string("host name", &host.name)?;
            // One registry of components, so a host shares no id or name with an editor or
            // another host, and one host per type, which a hosted block's lease names.
            if self.hosts[..index].iter().any(|other| {
                other.id == host.id || other.name == host.name || other.edits == host.edits
            }) || self
                .editors
                .iter()
                .any(|editor| editor.id == host.id || editor.name == host.name)
            {
                return Err(ManifestError::DuplicateHost);
            }
            if !host
                .levels
                .iter()
                .any(|level| level.access == OfferedAccess::Join)
                || host.levels.iter().enumerate().any(|(index, level)| {
                    host.levels[..index]
                        .iter()
                        .any(|other| other.access == level.access)
                })
            {
                return Err(ManifestError::InvalidLevels);
            }
            for level in &host.levels {
                manifest_string("level name", &level.name)?;
                manifest_string("level line", &level.line)?;
            }
        }
        manifest_string("entry point", &self.entry_point)?;
        for host in &self.network {
            manifest_string("network host", host)?;
            if host != ANY_HOST
                && !host
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
            {
                return Err(ManifestError::InvalidNetworkHost);
            }
        }
        for (index, worker) in self.workers.iter().enumerate() {
            manifest_string("worker", worker)?;
            if !plain_module(worker)
                || *worker == self.entry_point
                || self.workers[..index].contains(worker)
            {
                return Err(ManifestError::InvalidWorker);
            }
        }
        for (index, extension) in self.extensions.iter().enumerate() {
            if !EXTENSIONS.contains(&extension.as_str())
                || self.extensions[..index].contains(extension)
            {
                return Err(ManifestError::UnknownExtension);
            }
        }
        Ok(())
    }
}

impl EditorManifest {
    fn validate(&self) -> Result<(), ManifestError> {
        manifest_string("editor name", &self.name)?;
        manifest_string("display name", &self.display_name)?;
        manifest_string("icon", &self.icon)?;
        if !self.views.contains(&ParentView::Main)
            || ParentView::ALL
                .iter()
                .any(|view| self.views.iter().filter(|it| *it == view).count() > 1)
        {
            return Err(ManifestError::InvalidViews);
        }
        if self.edits == Edits::Nothing && !self.templates.is_empty() {
            return Err(ManifestError::ServiceWithTemplates);
        }
        for (index, template) in self.templates.iter().enumerate() {
            manifest_string("template id", &template.id)?;
            manifest_string("template name", &template.name)?;
            manifest_string("template icon", &template.icon)?;
            if self.templates[..index]
                .iter()
                .any(|other| other.id == template.id)
            {
                return Err(ManifestError::DuplicateTemplate);
            }
        }
        Ok(())
    }
}

fn manifest_string(field: &'static str, value: &str) -> Result<(), ManifestError> {
    if value.is_empty() {
        Err(ManifestError::Empty(field))
    } else if value.len() > MAX_STRING_BYTES {
        Err(ManifestError::TooLong(field))
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests;

/// Whether `name` is a module's plain file name, such as `tiles.wasm`: no path, only ASCII
/// letters, digits, `.`, `-` and `_`, ending in `.wasm`.
pub fn plain_module(name: &str) -> bool {
    name.strip_suffix(".wasm").is_some_and(|stem| {
        !stem.is_empty()
            && !stem.starts_with('.')
            && stem
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    })
}
