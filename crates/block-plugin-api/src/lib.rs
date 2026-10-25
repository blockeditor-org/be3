pub use accesskit;
use bincode::Options;
use serde::{Deserialize, Serialize};
pub use serde_bytes::ByteBuf;
use std::fmt;

be_ops_bindgen::generate!({
    path: "wit",
    world: "editor-plugin",
    deps: ["../ui-types/wit"],
    uses: { "be3:ui/types@0.1.0": ::ui_types::types },
    with: {
        "uuid": [u8; 16],
        "editor-instance-id": crate::EditorInstanceId,
        "screen-id": crate::ScreenId,
        "description": crate::Description,
        "child-placements": crate::ChildPlacements,
        "commit-id": [u8; 32],
        "metadata-entries": crate::MetadataEntries,
        "child-id": crate::ChildId,
        "web-view-id": crate::WebViewId,
        "interaction-mode": crate::InteractionMode,
        "pan-mode": crate::PanMode,
        "editor-capabilities": crate::EditorCapabilities,
        "resize-mode": crate::ResizeMode,
        "child-status": crate::ChildStatus,
        "child-operations": crate::ChildOperations,
        "press": crate::Press,
        "block-type-descriptor": crate::BlockTypeDescriptor,
        "template-category": crate::TemplateCategory,
        "edits": crate::Edits,
    },
    copy: [
        "uuid", "editor-instance-id", "screen-id", "commit-id", "child-id", "web-view-id", "interaction-mode",
        "pan-mode", "editor-capabilities", "resize-mode", "child-operations", "press",
        "template-category", "edits",
    ],
    default: [
        "frame-chrome", "top-bar", "frame-spec", "size", "fonts", "screen-layout",
        "block-metadata", "artifact-state", "version-status",
        "menu-command", "menu-check", "menu-pin", "menu-heading", "block-filter", "child-preview",
        "catalog", "audio-status",
    ],
    inline: [
        "rename-command", "artifact-command", "simulate-access-command", "unlink-command",
        "delete-command", "move-command", "place-command", "app-menu-command", "adopt-command",
        "finish-command", "branch-command", "create-branch-command", "resolve-command",
        "undo-operation-command", "performance-duration", "performance-count", "view-zoom",
        "artifact-described", "rename-dialog", "creation-options", "chosen-block",
    ],
    own_debug: ["font-face"],
});

pub use be_ops as ops;
pub use blocks::{
    AccessLevel, ArtifactAction, ArtifactState, BlockCommand, BlockInfo, BlockLocation,
    BlockMetadata, BlockQuery, ConflictSide, HostPanel, StoredHead, VersionBranch, VersionChange,
    VersionChangeKind, VersionCommand, VersionCommit, VersionOperation, VersionStatus,
};
pub use host_services::{
    AccessGrant, BlockFilter, ImportedBlock, PickedBlock, Revision, ShellDialog,
};
pub use shell_types::{
    ArtifactDescription, AudioCommand, AudioSource, AudioStatus, BarAction, BlockPick, Catalog,
    ChildPreview, CreationProgress, DroppedFile, EditorDescriptor, FitArea, MenuCheck, MenuCommand,
    MenuHeading, MenuItem, MenuItemKind, MenuPin, PerformanceMeasurement, PreviewImage,
    SettingsProgress, Stopped, TemplateDescriptor, ViewChange, WebViewEvent,
};
pub use types::{
    EditorRegion, FontFace, FontRole, Fonts, FrameChrome, FrameCommit, FrameReport, FrameSpec,
    Hello, HelloAccepted, InputBatch, Monitor, OpenArtifact, OpenBlock, OpenCreation, Opening,
    PluginIdentity, PresentedFrame, PressClaim, RegionSize, ScreenDamage, ScreenLayout,
    ScreenPlacement, ScreenRequest, ScreenWindow, Size, SurfaceFormat, SurfaceRect, SurfaceSpec,
    SurfaceSupport, TimedInput, TopBar, ViewportMetrics,
};

mod accesskit_mirror;
mod block_ids;
mod host_value;
mod linux;
mod manifest;
mod nesting;
mod opening;
mod session;
mod watch;
pub use block_ids::BlockIdRole;
pub use host_value::{
    Administrator, Contacts, HostAction, HostContact, HostImage, HostPreferenceSources, HostValue,
    LinkBase, PreferenceSources, decode_host, encode_host,
};
pub use linux::{
    AudioOutput, Displays, Epaper, EpaperRequest, HostDisplay, HostInputDevice,
    HostNotificationAction, HostProgram, HostTrayItem, HostWindow, HostWindowId, HostWindows,
    IncomingNotification, InputDevices, Media, MediaLevel, MediaLevels, MediaRequest,
    NotificationCloseReason, NotificationInbox, NotificationReport, NotificationRequest,
    NotificationSignal, NotificationUrgency, Notifications, PlayerCommand, Power, PowerAction,
    PowerAvailability, ProgramAction, Programs, RemapState, Remaps, ScreenLocked, Tray,
    TrayMenuEntry, TrayRequest, TrayStatus, TrayToggle, Wifi, WifiLink, WifiNetwork, WifiRequest,
    WifiSecurity, WifiStatus, WindowAction, WindowOrigin,
};
pub use manifest::{
    EditorDocument, InOrder, ManifestDocument, TemplateDocument, manifest_from_json,
};
pub use session::{HostSession, QueueError, SessionFailure, SessionState};
pub use ui_types::geometry::{Pos2, Rect, Vec2};
pub use ui_types::{
    BackEdge, BackGesture, CursorIcon, FileFilter, FilePick, FileSave, ImeEvent, ImeText, Input,
    Key, KeyChord, Keyboard, Keystroke, Modifiers, PasteKind, Pasted, Pen, PenPhase, PickedFile,
    PointerButton, SavedFile, ScrollDelta, TouchId, TouchPhase,
};
pub use ui_types::{
    CanvasTouch, DisplayMode, InputDevice, Motion, PointerSettings, PointerSpeed, PreferenceLayer,
    Preferences, Rotation, ThemeKind,
};
pub use ui_types::{ImeArea, WindowRequests};
pub use watch::Watch;

pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_MENU_DEPTH: usize = 8;
pub const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_QUEUED_MESSAGES: usize = 256;
pub const MAX_CHILDREN: usize = 256;
pub const MAX_ACCESSIBILITY_NODES: usize = 64 * 1024;
pub const MAX_LISTED_BLOCKS: usize = 16 * 1024;
pub const REQUEST_TIMEOUT_MILLISECONDS: u64 = 5_000;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EditorInstanceId(pub u64);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ScreenId(pub u64);

impl TopBar {
    pub fn shown(self) -> bool {
        matches!(self, Self::Shown)
    }

    pub fn phone(self) -> bool {
        matches!(self, Self::Phone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Command(MenuCommand),
    Submenu(MenuSubmenu),
    Separator,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuSubmenu {
    pub id: String,
    pub label: String,
    pub glyph: String,
    pub entries: Vec<MenuEntry>,
}

impl MenuEntry {
    pub fn command(entries: &[MenuEntry], id: &str) -> Option<MenuCommand> {
        entries.iter().find_map(|entry| match entry {
            MenuEntry::Command(command) => (command.id == id).then(|| command.clone()),
            MenuEntry::Submenu(submenu) => MenuEntry::command(&submenu.entries, id),
            MenuEntry::Separator => None,
        })
    }

    pub fn flatten(entries: &[MenuEntry]) -> Vec<MenuItem> {
        fn flatten_into(entries: &[MenuEntry], depth: u32, items: &mut Vec<MenuItem>) {
            for entry in entries {
                let kind = match entry {
                    MenuEntry::Command(command) => MenuItemKind::Command(command.clone()),
                    MenuEntry::Separator => MenuItemKind::Separator,
                    MenuEntry::Submenu(submenu) => MenuItemKind::Submenu(MenuHeading {
                        id: submenu.id.clone(),
                        label: submenu.label.clone(),
                        glyph: submenu.glyph.clone(),
                    }),
                };
                items.push(MenuItem { depth, kind });
                if let MenuEntry::Submenu(submenu) = entry {
                    flatten_into(&submenu.entries, depth + 1, items);
                }
            }
        }
        let mut items = Vec::new();
        flatten_into(entries, 0, &mut items);
        items
    }

    pub fn nest(items: Vec<MenuItem>) -> Result<Vec<MenuEntry>, DecodeError> {
        collection(items.len())?;
        let mut open: Vec<Vec<MenuEntry>> = vec![Vec::new()];
        let mut headings: Vec<MenuHeading> = Vec::new();
        for item in items {
            let depth = item.depth as usize;
            let opens = matches!(item.kind, MenuItemKind::Submenu(_)) as usize;
            if depth + opens > MAX_MENU_DEPTH {
                return Err(DecodeError::LimitExceeded("menu depth"));
            }
            if depth >= open.len() {
                return Err(DecodeError::MalformedPayload);
            }
            while open.len() > depth + 1 {
                close_submenu(&mut open, &mut headings);
            }
            match item.kind {
                MenuItemKind::Command(command) => {
                    strings([&command.id, &command.label, &command.glyph])?;
                    open[depth].push(MenuEntry::Command(command));
                }
                MenuItemKind::Separator => open[depth].push(MenuEntry::Separator),
                MenuItemKind::Submenu(heading) => {
                    strings([&heading.id, &heading.label, &heading.glyph])?;
                    headings.push(heading);
                    open.push(Vec::new());
                }
            }
        }
        while open.len() > 1 {
            close_submenu(&mut open, &mut headings);
        }
        Ok(open.pop().unwrap_or_default())
    }
}

mod menu_items {
    use super::{MenuEntry, MenuItem};
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

    pub(super) fn serialize<S: Serializer>(
        entries: &[MenuEntry],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        MenuEntry::flatten(entries).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<MenuEntry>, D::Error> {
        MenuEntry::nest(Vec::<MenuItem>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

fn close_submenu(open: &mut Vec<Vec<MenuEntry>>, headings: &mut Vec<MenuHeading>) {
    let entries = open.pop().unwrap_or_default();
    let Some(heading) = headings.pop() else {
        return;
    };
    if let Some(parent) = open.last_mut() {
        parent.push(MenuEntry::Submenu(MenuSubmenu {
            id: heading.id,
            label: heading.label,
            glyph: heading.glyph,
            entries,
        }));
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Description {
    pub tree: Option<accesskit::TreeUpdate>,
    pub actions: Vec<DescribedAction>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DescribedAction {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub checked: Option<bool>,
    pub live: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenSet {
    pub request_id: u64,
    pub screens: Vec<ScreenRequest>,
}

impl ScreenPlacement {
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor_millis as f32 / 1000.0
    }
}

impl ScreenLayout {
    pub fn placed(screens: &[ScreenRequest], surface: impl FnMut(ScreenId) -> u32) -> Self {
        let mut surface = surface;
        Self {
            screens: screens
                .iter()
                .filter(|request| {
                    request.metrics.pixel_width > 0 && request.metrics.pixel_height > 0
                })
                .map(|request| ScreenPlacement {
                    screen: request.screen,
                    instance: request.instance,
                    region: request.region,
                    surface: surface(request.screen),
                    width: request.metrics.pixel_width,
                    height: request.metrics.pixel_height,
                    scale_factor_millis: (request.metrics.scale_factor * 1000.0).round().max(1.0)
                        as u32,
                })
                .collect(),
        }
    }

    pub fn placement(&self, screen: ScreenId) -> Option<&ScreenPlacement> {
        self.screens
            .iter()
            .find(|placement| placement.screen == screen)
    }

    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    pub fn same_placements(&self, other: &Self) -> bool {
        self.screens == other.screens
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildLayer {
    #[default]
    Below,
    Above,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildMode {
    Preview,
    #[default]
    Passive,
    Active,
    Live,
    InPlace,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebViewId(pub u32);

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildContent {
    Block {
        block_id: [u8; 16],
        block_type: [u8; 16],
        view_block: Option<[u8; 16]>,
        editor: Option<[u8; 16]>,
    },
    WebView(WebViewId),
    Host(HostPanel),
    Window(HostWindowId),
    Creation {
        editor: [u8; 16],
        template: String,
    },
    ArtifactSettings {
        block_id: [u8; 16],
    },
}

impl HostPanel {
    pub const ALL: [Self; 3] = [Self::BlockStack, Self::Performance, Self::Plugins];

    pub fn title(self) -> &'static str {
        match self {
            Self::BlockStack => "Block Stack State",
            Self::Performance => "Performance",
            Self::Plugins => "Plugins",
        }
    }
}

impl ChildContent {
    pub fn block_id(&self) -> Option<[u8; 16]> {
        match self {
            Self::Block { block_id, .. } => Some(*block_id),
            Self::WebView(_)
            | Self::Host(_)
            | Self::Window(_)
            | Self::Creation { .. }
            | Self::ArtifactSettings { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChildPlacement {
    pub child: ChildId,
    pub content: ChildContent,
    pub rect: Rect,
    pub clip: Rect,
    pub own_frame: bool,
    pub top_bar: TopBar,
    pub corner_radius: f32,
    pub layer: ChildLayer,
    pub mode: ChildMode,
    pub intrinsic: Option<Size>,
    pub rotation: f32,
    pub opacity: f32,
    pub follows_view: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Occluder {
    pub after: u32,
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChildPlacements {
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub children: Vec<ChildPlacement>,
    pub occluders: Vec<Occluder>,
}

impl ChildPlacements {
    pub fn occluded(&self, index: usize, point: Pos2) -> bool {
        self.occluders
            .iter()
            .filter(|occluder| occluder.after as usize > index)
            .any(|occluder| occluder.rect.contains_half_open(point))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChildStatus {
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub child: ChildId,
    pub available: bool,
    pub intrinsic: Option<Size>,
    pub aspect_ratio: Option<f32>,
    pub active: bool,
    pub interaction: InteractionMode,
    pub capabilities: EditorCapabilities,
    pub resize: ResizeMode,
    pub error: Option<String>,
    #[serde(with = "menu_items")]
    pub menu: Vec<MenuEntry>,
    pub creation: Option<CreationProgress>,
    pub settings: Option<SettingsProgress>,
    pub preview: ChildPreview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub identity: PluginIdentity,
    pub editors: Vec<EditorManifest>,
    pub block_types: Vec<BlockTypeManifest>,
    pub entry_point: String,
    pub network: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTypeManifest {
    pub name: String,
    pub descriptor: BlockTypeDescriptor,
    pub module: String,
    pub exported_whole: bool,
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
    pub regions: Vec<EditorRegion>,
    pub accepts: Vec<ShellRequest>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellRequest {
    ShowBlock,
    Pick,
    Dialog,
    Panel,
    Window,
}

impl ShellRequest {
    pub const ALL: [Self; 5] = [
        Self::ShowBlock,
        Self::Pick,
        Self::Dialog,
        Self::Panel,
        Self::Window,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateManifest {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub category: TemplateCategory,
    pub dialog: bool,
    pub block_type: [u8; 16],
}

#[derive(
    Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum TemplateCategory {
    Important,
    #[default]
    Regular,
    Debug,
    Template,
}

impl PluginManifest {
    pub fn editor(&self, id: [u8; 16]) -> Option<&EditorManifest> {
        self.editors.iter().find(|editor| editor.id == id)
    }

    pub fn type_module(&self) -> Option<&str> {
        self.block_types
            .first()
            .map(|block_type| block_type.module.as_str())
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edits {
    Type([u8; 16]),
    Any,
    Nothing,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChildOperations {
    pub add: bool,
    pub delete: bool,
    pub replace: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionMode {
    Preview,
    #[default]
    Live,
    Playback,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCapabilities {
    pub rotation: bool,
    pub preserve_aspect_ratio: bool,
    pub pan_and_zoom: bool,
    #[serde(default)]
    pub max_zoom: Option<u32>,
    #[serde(default)]
    pub pan: PanMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanMode {
    None,
    OneFinger,
    #[default]
    TwoFinger,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResizeMode {
    None,
    Horizontal,
    Vertical,
    #[default]
    Both,
}

impl ResizeMode {
    pub fn horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    pub fn vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTypeDescriptor {
    pub block_type: [u8; 16],
    pub display_name: String,
    pub color: Option<[u8; 3]>,
    pub icon_codepoint: String,
    pub children: ChildOperations,
    pub press: Press,
    pub plain_file: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Press {
    #[default]
    Open,
    Zoom,
}

impl EditorRegion {
    pub const ALL: [Self; 3] = [Self::Frame, Self::Preview, Self::ArtifactSettings];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    Malformed(String),
    Empty(&'static str),
    TooLong(&'static str),
    InvalidIdentity,
    InvalidBlockType,
    InvalidEditorId,
    InvalidRegions,
    InvalidNetworkHost,
    NoEditors,
    DuplicateEditor,
    DuplicateBlockType,
    SeveralTypeModules,
    DuplicateTemplate,
    TemplateWithoutBlockType,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => write!(formatter, "the manifest could not be read: {error}"),
            Self::Empty(field) => write!(formatter, "{field} is empty"),
            Self::TooLong(field) => write!(formatter, "{field} is too long"),
            Self::InvalidIdentity => formatter.write_str("the plugin id is not a plain identifier"),
            Self::InvalidBlockType => formatter.write_str("the block type is not a uuid"),
            Self::InvalidEditorId => formatter.write_str("the editor id is not a uuid"),
            Self::InvalidRegions => {
                formatter.write_str("the regions must include the frame exactly once")
            }
            Self::InvalidNetworkHost => {
                formatter.write_str("a network host is not a plain host name")
            }
            Self::NoEditors => {
                formatter.write_str("the plugin declares no editors and no block types")
            }
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
        }
    }
}

impl PluginManifest {
    pub fn validate(&self) -> Result<(), ManifestError> {
        manifest_string("plugin id", &self.identity.id)?;
        manifest_string("plugin name", &self.identity.name)?;
        manifest_string("plugin version", &self.identity.version)?;
        if !self
            .identity
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(ManifestError::InvalidIdentity);
        }
        if self.editors.is_empty() && self.block_types.is_empty() {
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
        manifest_string("entry point", &self.entry_point)?;
        for host in &self.network {
            manifest_string("network host", host)?;
            if !host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
            {
                return Err(ManifestError::InvalidNetworkHost);
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
        if !self.regions.contains(&EditorRegion::Frame)
            || EditorRegion::ALL
                .iter()
                .any(|region| self.regions.iter().filter(|it| *it == region).count() > 1)
        {
            return Err(ManifestError::InvalidRegions);
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EditorMessage {
    Open {
        instance: EditorInstanceId,
        editor: [u8; 16],
        block_id: [u8; 16],
        block_type: [u8; 16],
        view_block: Option<[u8; 16]>,
        account_id: [u8; 16],
        organization_id: [u8; 16],
        client_id: [u8; 16],
        editable: bool,
    },

    WatchContent {
        instance: EditorInstanceId,
        blocks: Vec<WatchedContent>,
    },
    SeedContent {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        content_type: [u8; 16],
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    ReplaceContent {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        content_type: [u8; 16],
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    Close {
        instance: EditorInstanceId,
    },

    OpenBlock {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        block_type: [u8; 16],
        via: Option<[u8; 16]>,
        editor: Option<[u8; 16]>,
    },

    ShowBlock {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        block_type: [u8; 16],
        via: Option<[u8; 16]>,
        editor: Option<[u8; 16]>,
    },

    ShowPanel {
        instance: EditorInstanceId,
        panel: HostPanel,
    },

    BlockCommand {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        command: BlockCommand,
    },

    OpenCreation {
        instance: EditorInstanceId,
        editor: [u8; 16],
        template: String,
        account_id: [u8; 16],
        organization_id: [u8; 16],
        client_id: [u8; 16],
    },
    OpenArtifact {
        instance: EditorInstanceId,
        editor: [u8; 16],
        block_id: [u8; 16],
        block_type: [u8; 16],
        account_id: [u8; 16],
        organization_id: [u8; 16],
        client_id: [u8; 16],
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    WatchArtifacts {
        instance: EditorInstanceId,
        blocks: Vec<[u8; 16]>,
    },
    ArtifactStates {
        instance: EditorInstanceId,
        states: Vec<ArtifactState>,
    },
    WatchHistory {
        instance: EditorInstanceId,
        blocks: Vec<[u8; 16]>,
    },
    HistoryStates {
        instance: EditorInstanceId,
        states: Vec<HistoryState>,
    },
    WatchHeads {
        instance: EditorInstanceId,
        blocks: Vec<[u8; 16]>,
    },
    Head {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        head: Result<Option<StoredHead>, String>,
    },

    WatchBlocks {
        instance: EditorInstanceId,
        queries: Vec<BlockQuery>,
    },
    Blocks {
        instance: EditorInstanceId,
        query: BlockQuery,
        blocks: Vec<BlockInfo>,
    },
    BlocksFailed {
        instance: EditorInstanceId,
        query: BlockQuery,
        error: String,
    },
    CreateBlock {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        content_type: [u8; 16],
        parent: BlockLocation,
        defined: MetadataEntries,
        content: Option<serde_bytes::ByteBuf>,
    },
    SetParent {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        parent: BlockLocation,
    },
    Define {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        key: [u8; 16],
        value: Option<serde_bytes::ByteBuf>,
    },
    VersionControl {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        command: VersionCommand,
    },
    VersionStatus {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        status: VersionStatus,
    },
    Replica {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        #[serde(with = "serde_bytes")]
        frame: Vec<u8>,
    },
}

impl EditorMessage {
    pub fn instance(&self) -> EditorInstanceId {
        match self {
            Self::Open { instance, .. }
            | Self::WatchContent { instance, .. }
            | Self::SeedContent { instance, .. }
            | Self::ReplaceContent { instance, .. }
            | Self::Close { instance, .. }
            | Self::OpenBlock { instance, .. }
            | Self::ShowBlock { instance, .. }
            | Self::ShowPanel { instance, .. }
            | Self::BlockCommand { instance, .. }
            | Self::OpenCreation { instance, .. }
            | Self::OpenArtifact { instance, .. }
            | Self::WatchArtifacts { instance, .. }
            | Self::ArtifactStates { instance, .. }
            | Self::WatchHistory { instance, .. }
            | Self::HistoryStates { instance, .. }
            | Self::WatchHeads { instance, .. }
            | Self::Head { instance, .. }
            | Self::WatchBlocks { instance, .. }
            | Self::Blocks { instance, .. }
            | Self::BlocksFailed { instance, .. }
            | Self::CreateBlock { instance, .. }
            | Self::SetParent { instance, .. }
            | Self::Define { instance, .. }
            | Self::VersionControl { instance, .. }
            | Self::VersionStatus { instance, .. }
            | Self::Replica { instance, .. } => *instance,
        }
    }
}

pub type MetadataEntries = std::collections::BTreeMap<[u8; 16], serde_bytes::ByteBuf>;

impl Default for AccessLevel {
    fn default() -> Self {
        Self::Edit
    }
}

impl AccessLevel {
    pub fn can_know_exists(self) -> bool {
        self >= Self::KnowExists
    }

    pub fn can_view(self) -> bool {
        self >= Self::View
    }

    pub fn can_edit(self) -> bool {
        self == Self::Edit
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "No access",
            Self::KnowExists => "Knows it exists",
            Self::View => "Can view",
            Self::Edit => "Can edit",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchedContent {
    pub block_id: [u8; 16],
    pub content_type: [u8; 16],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryState {
    pub head: Option<[u8; 32]>,
    pub block_id: [u8; 16],
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebViewCommand {
    Open(String),
    Load(String),
    Reload,
    FocusApp,
    Close,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    Hello(Hello),
    HelloAccepted(HelloAccepted),
    Preferences(Preferences),
    UtcOffset(i32),
    Fonts(Fonts),
    MissingCharacters(Vec<char>),
    Screens(ScreenSet),
    Configure { serial: u64 },
    Layout(ScreenLayout),
    Input(InputBatch),
    DrawFrame { now_micros: u64, seen: bool },
    FrameNeeded,
    Frame(FrameCommit),
    Acknowledged { request_id: u64 },
    Error(ProtocolError),
    Shutdown,
    ShutdownAcknowledged,
    Editor(EditorMessage),
}

impl Message {
    pub fn is_session(&self) -> bool {
        matches!(
            self,
            Self::Hello(_)
                | Self::HelloAccepted(_)
                | Self::Acknowledged { .. }
                | Self::Error(_)
                | Self::Shutdown
                | Self::ShutdownAcknowledged
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    ToPlugin,
    ToHost,
    Either,
}

impl Message {
    pub fn direction(&self) -> Direction {
        match self {
            Self::HelloAccepted(_)
            | Self::Preferences(_)
            | Self::UtcOffset(_)
            | Self::Fonts(_)
            | Self::Screens(_)
            | Self::Configure { .. }
            | Self::Input(_)
            | Self::DrawFrame { .. }
            | Self::Shutdown => Direction::ToPlugin,
            Self::Hello(_)
            | Self::Acknowledged { .. }
            | Self::ShutdownAcknowledged
            | Self::Layout(_)
            | Self::MissingCharacters(_)
            | Self::FrameNeeded
            | Self::Frame(_) => Direction::ToHost,
            Self::Error(_) => Direction::Either,
            Self::Editor(editor) => editor.direction(),
        }
    }
}

impl EditorMessage {
    pub fn direction(&self) -> Direction {
        match self {
            Self::Open { .. }
            | Self::OpenCreation { .. }
            | Self::OpenArtifact { .. }
            | Self::Close { .. }
            | Self::ShowBlock { .. }
            | Self::ShowPanel { .. }
            | Self::ArtifactStates { .. }
            | Self::HistoryStates { .. }
            | Self::Head { .. }
            | Self::Blocks { .. }
            | Self::BlocksFailed { .. }
            | Self::VersionStatus { .. } => Direction::ToPlugin,
            Self::OpenBlock { .. }
            | Self::BlockCommand { .. }
            | Self::WatchArtifacts { .. }
            | Self::WatchHistory { .. }
            | Self::WatchHeads { .. }
            | Self::WatchContent { .. }
            | Self::SeedContent { .. }
            | Self::ReplaceContent { .. }
            | Self::WatchBlocks { .. }
            | Self::VersionControl { .. }
            | Self::CreateBlock { .. }
            | Self::SetParent { .. }
            | Self::Define { .. } => Direction::ToHost,
            Self::Replica { .. } => Direction::Either,
        }
    }
}

impl fmt::Debug for FontFace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontFace")
            .field("role", &self.role)
            .field("index", &self.index)
            .field("bytes", &self.data.len())
            .finish()
    }
}

impl FitArea {
    pub fn fit(self, world: Vec2, room: Vec2) -> (f32, Pos2) {
        let area = match self {
            Self::World => Rect::from_min_size(Pos2::ZERO, world),
            Self::Centre => Rect::from_min_size(Pos2::ZERO + world * 0.5, Vec2::ZERO),
            Self::Rect(rect) => rect,
        };
        let size = area.size();
        let scale = (room.x.max(1.0) / size.x.max(f32::EPSILON))
            .min(room.y.max(1.0) / size.y.max(f32::EPSILON))
            .min(1.0);
        (scale, area.center())
    }
}

impl From<Input> for TimedInput {
    fn from(input: Input) -> Self {
        Self {
            input,
            time_micros: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolError {
    pub request_id: Option<u64>,
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    InvalidMessage,
    InvalidState,
    Internal,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    TruncatedFrame { expected: usize, available: usize },
    MalformedPayload,
    LimitExceeded(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DecodeError {}

pub fn encode_frame(message: &Message) -> Result<Vec<u8>, DecodeError> {
    validate(message)?;
    let payload = codec()
        .serialize(message)
        .map_err(|_| DecodeError::MalformedPayload)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_frame(frame: &[u8]) -> Result<Message, DecodeError> {
    if frame.len() < 4 {
        return Err(DecodeError::TruncatedFrame {
            expected: 4,
            available: frame.len(),
        });
    }
    let length = u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize;
    if frame.len() != length + 4 {
        return Err(DecodeError::TruncatedFrame {
            expected: length + 4,
            available: frame.len(),
        });
    }
    let message = codec()
        .deserialize(&frame[4..])
        .map_err(|error| match *error {
            bincode::ErrorKind::Custom(message) if message == nesting::TOO_DEEP => {
                DecodeError::LimitExceeded("menu depth")
            }
            _ => DecodeError::MalformedPayload,
        })?;
    validate(&message)?;
    Ok(message)
}

fn validate(message: &Message) -> Result<(), DecodeError> {
    match message {
        Message::Hello(value) => {
            strings([&value.plugin.id, &value.plugin.name, &value.plugin.version])
        }
        Message::HelloAccepted(value) => string(&value.host_name),
        Message::Error(value) => string(&value.message),
        Message::Input(value) => {
            collection(value.events.len())?;
            for event in &value.events {
                match &event.input {
                    Input::Paste(Pasted::Image(file)) => {
                        string(&file.name)?;
                        blob(&file.data)?;
                    }
                    Input::Text(value)
                    | Input::Key(Keystroke {
                        text: Some(value), ..
                    })
                    | Input::Paste(Pasted::Text(value))
                    | Input::Ime(
                        ImeEvent::SetComposingText(value)
                        | ImeEvent::CommitText(value)
                        | ImeEvent::ReplaceText { text: value, .. },
                    ) => text(value)?,
                    _ => {}
                }
            }
            Ok(())
        }
        Message::Screens(value) => collection(value.screens.len()),
        Message::Frame(value) => {
            collection(value.sizes.len())?;
            collection(value.reports.len())?;
            collection(value.windows.len())?;
            collection(value.children.len())?;
            if let Some(presented) = &value.presented {
                collection(presented.damage.len())?;
            }
            for screen in &value.windows {
                window(&screen.window)?;
            }
            for placements in &value.children {
                validate_children(placements)?;
            }
            for report in &value.reports {
                collection(report.painted.len())?;
                collection(report.floating.len())?;
                collection(report.drawing.len())?;
                collection(report.claims.len())?;
                if let Some(tree) = report
                    .description
                    .as_ref()
                    .and_then(|description| description.tree.as_ref())
                    && tree.nodes.len() > MAX_ACCESSIBILITY_NODES
                {
                    return Err(DecodeError::LimitExceeded("accessibility nodes"));
                }
            }
            Ok(())
        }
        Message::Layout(value) => collection(value.screens.len()),
        Message::Fonts(value) => {
            collection(value.faces.len())?;
            for face in &value.faces {
                blob(&face.data)?;
            }
            Ok(())
        }
        Message::MissingCharacters(value) => collection(value.len()),
        Message::Editor(value) => validate_editor(value),
        _ => Ok(()),
    }
}

fn window(window: &WindowRequests) -> Result<(), DecodeError> {
    collection(window.intercepted_keys.len())?;
    collection(window.pastes.len())?;
    match &window.ime {
        Some(ImeArea {
            text: Some(surrounding),
            ..
        }) => text(&surrounding.text),
        _ => Ok(()),
    }
}

fn validate_children(placements: &ChildPlacements) -> Result<(), DecodeError> {
    if placements.children.len() > MAX_CHILDREN {
        return Err(DecodeError::LimitExceeded("children"));
    }
    if placements.occluders.len() > MAX_COLLECTION_ITEMS {
        return Err(DecodeError::LimitExceeded("occluders"));
    }
    let mut covered = 0;
    for occluder in &placements.occluders {
        if occluder.after as usize > placements.children.len()
            || (occluder.after as usize) < covered
        {
            return Err(DecodeError::MalformedPayload);
        }
        covered = occluder.after as usize;
    }
    for child in &placements.children {
        if let ChildContent::Creation { template, .. } = &child.content {
            string(template)?;
        }
    }
    Ok(())
}

fn validate_editor(message: &EditorMessage) -> Result<(), DecodeError> {
    match message {
        EditorMessage::OpenCreation { template, .. } => string(template),
        EditorMessage::OpenArtifact { data, .. } => descriptor(data),
        EditorMessage::WatchArtifacts { blocks, .. }
        | EditorMessage::WatchHistory { blocks, .. } => collection(blocks.len()),
        EditorMessage::HistoryStates { states, .. } => collection(states.len()),
        EditorMessage::WatchHeads { blocks, .. } => collection(blocks.len()),
        EditorMessage::Head { head, .. } => match head {
            Ok(Some(head)) => blob(&head.bytes),
            Ok(None) => Ok(()),
            Err(error) => string(error),
        },
        EditorMessage::Replica { frame, .. } => blob(frame),
        EditorMessage::WatchContent { blocks, .. } => collection(blocks.len()),
        EditorMessage::WatchBlocks { queries, .. } => collection(queries.len()),
        EditorMessage::BlocksFailed { error, .. } => string(error),
        EditorMessage::Blocks { blocks, .. } => {
            if blocks.len() > MAX_LISTED_BLOCKS {
                return Err(DecodeError::LimitExceeded("block list"));
            }
            for block in blocks {
                collection(block.references.len())?;
                metadata(&block.metadata.defined)?;
                metadata(&block.metadata.derived)?;
            }
            Ok(())
        }
        EditorMessage::CreateBlock {
            defined, content, ..
        } => {
            metadata(defined)?;
            content.as_ref().map_or(Ok(()), |content| blob(content))
        }
        EditorMessage::Define {
            value: Some(value), ..
        } => descriptor(value),
        EditorMessage::VersionControl { command, .. } => match command {
            VersionCommand::Finish { description: value }
            | VersionCommand::Switch { branch: value }
            | VersionCommand::CreateBranch { name: value }
            | VersionCommand::Merge { branch: value }
            | VersionCommand::NewCheckout { branch: value } => string(value),
            VersionCommand::Adopt { .. }
            | VersionCommand::Update
            | VersionCommand::Resolve { .. }
            | VersionCommand::Undo { .. }
            | VersionCommand::Fork
            | VersionCommand::PullUpstream
            | VersionCommand::PushUpstream => Ok(()),
        },
        EditorMessage::VersionStatus { status, .. } => {
            if let Some(error) = &status.error {
                string(error)?;
            }
            collection(status.branches.len())?;
            strings(status.branches.iter().map(|branch| &branch.name))?;
            if status.changes.len() > MAX_LISTED_BLOCKS {
                return Err(DecodeError::LimitExceeded("changes"));
            }
            strings(
                status
                    .changes
                    .iter()
                    .filter_map(|change| change.name.as_ref()),
            )?;
            collection(status.log.len())?;
            for commit in status.log.iter().chain(&status.working) {
                collection(commit.parents.len())?;
                string(&commit.message)?;
            }
            collection(status.operations.len())?;
            strings(
                status
                    .operations
                    .iter()
                    .map(|operation| &operation.description),
            )?;
            Ok(())
        }
        EditorMessage::ArtifactStates { states, .. } => {
            collection(states.len())?;
            for state in states {
                string(&state.summary)?;
                if let Some(error) = &state.error {
                    string(error)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn descriptor(data: &[u8]) -> Result<(), DecodeError> {
    if data.len() > MAX_OPAQUE_DESCRIPTOR_BYTES {
        Err(DecodeError::LimitExceeded("artifact settings"))
    } else {
        Ok(())
    }
}

fn metadata(entries: &MetadataEntries) -> Result<(), DecodeError> {
    collection(entries.len())?;
    entries.values().try_for_each(|value| descriptor(value))
}

fn collection(length: usize) -> Result<(), DecodeError> {
    if length > MAX_COLLECTION_ITEMS {
        Err(DecodeError::LimitExceeded("collection"))
    } else {
        Ok(())
    }
}

fn string(value: &str) -> Result<(), DecodeError> {
    if value.len() > MAX_STRING_BYTES {
        Err(DecodeError::LimitExceeded("string"))
    } else {
        Ok(())
    }
}

pub fn paste_events(text: &str) -> Vec<Input> {
    text_pieces(text, MAX_TEXT_BYTES)
        .into_iter()
        .map(|piece| Input::Paste(Pasted::Text(piece)))
        .collect()
}

fn text_pieces(text: &str, limit: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let mut end = rest.len().min(limit);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let (piece, after) = rest.split_at(end);
        pieces.push(piece.to_owned());
        rest = after;
    }
    pieces
}

fn text(value: &str) -> Result<(), DecodeError> {
    if value.len() > MAX_TEXT_BYTES {
        Err(DecodeError::LimitExceeded("text"))
    } else {
        Ok(())
    }
}

fn blob(value: &[u8]) -> Result<(), DecodeError> {
    if value.len() > MAX_BLOB_BYTES {
        Err(DecodeError::LimitExceeded("blob"))
    } else {
        Ok(())
    }
}

fn strings<'a>(values: impl IntoIterator<Item = &'a String>) -> Result<(), DecodeError> {
    for value in values {
        string(value)?;
    }
    Ok(())
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
}

#[cfg(test)]
mod tests;
