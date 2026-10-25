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
    },
    copy: ["uuid", "editor-instance-id", "screen-id", "commit-id", "child-id"],
    default: [
        "frame-chrome", "top-bar", "frame-spec", "size", "fonts", "screen-layout",
        "block-metadata", "artifact-state", "version-status",
        "menu-command", "menu-check", "menu-pin", "menu-heading",
    ],
    inline: [
        "rename-command", "artifact-command", "simulate-access-command", "unlink-command",
        "delete-command", "move-command", "place-command", "app-menu-command", "adopt-command",
        "finish-command", "branch-command", "create-branch-command", "resolve-command",
        "undo-operation-command", "performance-duration", "performance-count", "view-zoom", "artifact-described",
    ],
    own_debug: ["font-face"],
});

pub use be_ops as ops;
pub use blocks::{
    AccessLevel, ArtifactAction, ArtifactState, BlockCommand, BlockInfo, BlockLocation,
    BlockMetadata, BlockQuery, ConflictSide, HostPanel, StoredHead, VersionBranch, VersionChange,
    VersionChangeKind, VersionCommand, VersionCommit, VersionOperation, VersionStatus,
};
pub use shell_types::{
    ArtifactDescription, BarAction, DroppedFile, FitArea, MenuCheck, MenuCommand, MenuHeading,
    MenuItem, MenuItemKind, MenuPin, PerformanceMeasurement, Stopped, ViewChange,
};
pub use types::{
    EditorRegion, FontFace, FontRole, Fonts, FrameChrome, FrameCommit, FrameReport, FrameSpec,
    Hello, HelloAccepted, InputBatch, Monitor, OpenArtifact, OpenBlock, OpenCreation, Opening,
    PluginIdentity, PresentedFrame, PressClaim, RegionSize, ScreenDamage, ScreenLayout,
    ScreenPlacement, ScreenRequest, ScreenWindow, Size, SurfaceFormat, SurfaceRect, SurfaceSpec,
    SurfaceSupport, TimedInput, TopBar, ViewportMetrics,
};

mod block_ids;
mod host_value;
mod linux;
mod manifest;
mod nesting;
mod opening;
mod session;
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuEntry {
    Command(MenuCommand),
    Submenu(MenuSubmenu),
    Separator,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuSubmenu {
    pub id: String,
    pub label: String,
    pub glyph: String,
    #[serde(deserialize_with = "nesting::submenu")]
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
    pub menu: Vec<MenuEntry>,
    pub creation: Option<CreationProgress>,
    pub settings: Option<SettingsProgress>,
    pub preview: ChildPreview,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildPreview {
    #[default]
    Hosted,
    Waiting,
    Label,
    Image(PreviewImage),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PreviewImage {
    pub image: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsProgress {
    pub changed: bool,
    pub summary: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShellDialog {
    Rename {
        anchor: Option<Rect>,
        within: Option<[u8; 16]>,
    },
    Share,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessGrant {
    pub account: [u8; 16],
    pub email: String,
    pub display_name: String,
    pub administrator: bool,
    pub author: bool,
    pub granted: Option<AccessLevel>,
    pub effective: AccessLevel,
    pub waiting: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessListing {
    Listed(Vec<AccessGrant>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContactLinkResult {
    Made(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationProgress {
    Options { ready: bool },
    Working,
    Created([u8; 16]),
    Failed(String),
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateDescriptor {
    pub editor: [u8; 16],
    pub template: String,
    pub block_type: [u8; 16],
    pub name: String,
    pub icon_codepoint: String,
    pub category: TemplateCategory,
    pub dialog: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub types: Vec<BlockTypeDescriptor>,
    pub templates: Vec<TemplateDescriptor>,
    pub editors: Vec<EditorDescriptor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorDescriptor {
    pub editor: [u8; 16],
    pub edits: Edits,
    pub name: String,
    pub icon_codepoint: String,
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

    WatchHostValue {
        instance: EditorInstanceId,
        key: String,
    },

    HostValue {
        instance: EditorInstanceId,
        key: String,
        #[serde(with = "serde_bytes")]
        value: Vec<u8>,
    },

    HostAction {
        instance: EditorInstanceId,
        key: String,
        #[serde(with = "serde_bytes")]
        action: Vec<u8>,
    },

    ShowDialog {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        dialog: ShellDialog,
    },

    SetAccess {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        account: [u8; 16],
        access: AccessLevel,
    },

    BlockCommand {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        command: BlockCommand,
    },

    Request {
        instance: EditorInstanceId,
        request_id: u64,
        request: HostRequest,
    },
    Replied {
        instance: EditorInstanceId,
        request_id: u64,
        reply: HostReply,
    },
    PlayAudio {
        instance: EditorInstanceId,
        command: AudioCommand,
    },
    AudioStatus {
        instance: EditorInstanceId,
        status: AudioStatus,
    },
    WebViewCommand {
        instance: EditorInstanceId,
        web_view: WebViewId,
        command: WebViewCommand,
    },
    WebViewEvent {
        instance: EditorInstanceId,
        web_view: WebViewId,
        event: WebViewEvent,
    },
    OpenCreation {
        instance: EditorInstanceId,
        editor: [u8; 16],
        template: String,
        account_id: [u8; 16],
        organization_id: [u8; 16],
        client_id: [u8; 16],
    },
    PickRequested {
        instance: EditorInstanceId,
        pick: u64,
        filter: BlockFilter,
        parent: BlockLocation,
        within: Option<[u8; 16]>,
        touch: bool,
    },
    PickAnswered {
        instance: EditorInstanceId,
        pick: u64,
        answer: BlockPick,
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
            | Self::WatchHostValue { instance, .. }
            | Self::HostValue { instance, .. }
            | Self::HostAction { instance, .. }
            | Self::ShowDialog { instance, .. }
            | Self::SetAccess { instance, .. }
            | Self::BlockCommand { instance, .. }
            | Self::Request { instance, .. }
            | Self::Replied { instance, .. }
            | Self::PlayAudio { instance, .. }
            | Self::AudioStatus { instance, .. }
            | Self::WebViewCommand { instance, .. }
            | Self::WebViewEvent { instance, .. }
            | Self::OpenCreation { instance, .. }
            | Self::PickRequested { instance, .. }
            | Self::PickAnswered { instance, .. }
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
pub struct Revision {
    pub commit: [u8; 32],
    pub time: i64,
    pub current: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum HostRequest {
    ListRevisions([u8; 16]),
    PreviewRevision {
        block_id: [u8; 16],
        commit: [u8; 32],
    },
    RestoreRevision {
        block_id: [u8; 16],
        commit: [u8; 32],
    },
    ReleaseRevision([u8; 16]),
    PickFile(FileFilter),
    SaveFile(SavedFile),
    PickBlock(BlockFilter),
    ListAccess([u8; 16]),
    Fetch(String),
    ListData,
    ReadData(String),
    ExportBlock([u8; 16]),
    ReadPlainFile([u8; 16]),
    ImportBlocks(BlockLocation),
    ContactLink {
        block_id: [u8; 16],
        access: AccessLevel,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostReply {
    RevisionsListed(Result<Vec<Revision>, String>),
    RevisionPreviewed(Result<[u8; 16], String>),
    RevisionRestored(Result<(), String>),
    RevisionReleased,

    FilePicked(FilePick),
    FileSaved(FileSave),
    PlainFileRead(Result<SavedFile, String>),
    BlockPicked(BlockPick),
    Fetched(FetchResult),
    DataListed(DataListing),
    DataRead(FetchResult),
    AccessListed(AccessListing),
    BlocksImported(BlockImport),
    ContactLinkMade(ContactLinkResult),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockImport {
    Imported {
        block_id: [u8; 16],
        block_type: [u8; 16],
    },
    Cancelled,
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BlockFilter {
    pub name: String,
    pub block_types: Vec<[u8; 16]>,
    pub excluded: Vec<[u8; 16]>,
    pub templates: bool,
    pub place: Option<BlockLocation>,
    pub anchor: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockPick {
    Chosen {
        block_id: [u8; 16],
        block_type: [u8; 16],
        linked: bool,
        placed: bool,
    },
    Cancelled,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FetchResult {
    Body(Vec<u8>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataListing {
    Files(Vec<String>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebViewCommand {
    Open(String),
    Load(String),
    Reload,
    FocusApp,
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebViewEvent {
    Navigate(String),
    Finished(String),
    Push(String),
    Replace(String),
    Title(String),
    History(i32),
    NewWindow(String),
    Address(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCommand {
    Load(AudioSource),
    Toggle,
    Reset,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSource {
    pub media_type: String,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioStatus {
    pub playing: bool,
    pub position_micros: u64,
    pub duration_micros: Option<u64>,
    pub error: Option<String>,
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
    Configure {
        serial: u64,
    },
    Layout(ScreenLayout),
    Input(InputBatch),
    DrawFrame {
        now_micros: u64,
        seen: bool,
    },
    FrameNeeded,
    Frame(FrameCommit),
    Acknowledged {
        request_id: u64,
    },
    Error(ProtocolError),
    Shutdown,
    ShutdownAcknowledged,
    Editor(EditorMessage),
    BlockTypes(Catalog),
    ChildStatuses(Vec<ChildStatus>),
    Describe(bool),
    RunAction {
        screen: ScreenId,
        id: String,
    },
    NodeAction {
        screen: ScreenId,
        request: accesskit::ActionRequest,
    },
    Extension {
        key: [u8; 16],
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
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
            | Self::Shutdown
            | Self::BlockTypes(_)
            | Self::ChildStatuses(_)
            | Self::Describe(_)
            | Self::RunAction { .. }
            | Self::NodeAction { .. } => Direction::ToPlugin,
            Self::Hello(_)
            | Self::Acknowledged { .. }
            | Self::ShutdownAcknowledged
            | Self::Layout(_)
            | Self::MissingCharacters(_)
            | Self::FrameNeeded
            | Self::Frame(_) => Direction::ToHost,
            Self::Error(_) | Self::Extension { .. } => Direction::Either,
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
            | Self::ShowDialog { .. }
            | Self::Replied { .. }
            | Self::AudioStatus { .. }
            | Self::WebViewEvent { .. }
            | Self::PickRequested { .. }
            | Self::ArtifactStates { .. }
            | Self::HistoryStates { .. }
            | Self::Head { .. }
            | Self::Blocks { .. }
            | Self::BlocksFailed { .. }
            | Self::HostValue { .. }
            | Self::VersionStatus { .. } => Direction::ToPlugin,
            Self::OpenBlock { .. }
            | Self::BlockCommand { .. }
            | Self::Request { .. }
            | Self::PlayAudio { .. }
            | Self::SetAccess { .. }
            | Self::PickAnswered { .. }
            | Self::WebViewCommand { .. }
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
            | Self::WatchHostValue { .. }
            | Self::HostAction { .. }
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
        Message::BlockTypes(value) => {
            collection(value.types.len())?;
            collection(value.templates.len())?;
            for descriptor in &value.types {
                string(&descriptor.display_name)?;
                string(&descriptor.icon_codepoint)?;
            }
            for template in &value.templates {
                string(&template.template)?;
                string(&template.name)?;
                string(&template.icon_codepoint)?;
            }
            collection(value.editors.len())?;
            for editor in &value.editors {
                string(&editor.name)?;
                string(&editor.icon_codepoint)?;
            }
            Ok(())
        }
        Message::RunAction { id, .. } => string(id),
        Message::Extension { payload, .. } => blob(payload),
        Message::NodeAction { request, .. } => match &request.data {
            Some(accesskit::ActionData::Value(value)) => text(value),
            _ => Ok(()),
        },
        Message::ChildStatuses(value) => {
            collection(value.len())?;
            for status in value {
                if let Some(error) = &status.error {
                    string(error)?;
                }
                menu(&status.menu)?;
                if let Some(CreationProgress::Failed(error)) = &status.creation {
                    string(error)?;
                }
                if let Some(summary) = status
                    .settings
                    .as_ref()
                    .and_then(|settings| settings.summary.as_ref())
                {
                    string(summary)?;
                }
            }
            Ok(())
        }
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

fn block_filter(filter: &BlockFilter) -> Result<(), DecodeError> {
    string(&filter.name)?;
    collection(filter.block_types.len())?;
    collection(filter.excluded.len())
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
        EditorMessage::Request { request, .. } => validate_request(request),
        EditorMessage::Replied { reply, .. } => validate_reply(reply),
        EditorMessage::PickRequested { filter, .. } => block_filter(filter),
        EditorMessage::PickAnswered { answer, .. } => match answer {
            BlockPick::Failed(error) => string(error),
            BlockPick::Chosen { .. } | BlockPick::Cancelled => Ok(()),
        },
        EditorMessage::OpenCreation { template, .. } => string(template),
        EditorMessage::OpenArtifact { data, .. } => descriptor(data),
        EditorMessage::AudioStatus { status, .. } => match &status.error {
            Some(message) => string(message),
            None => Ok(()),
        },
        EditorMessage::WatchArtifacts { blocks, .. }
        | EditorMessage::WatchHistory { blocks, .. } => collection(blocks.len()),
        EditorMessage::HistoryStates { states, .. } => collection(states.len()),
        EditorMessage::WatchHeads { blocks, .. } => collection(blocks.len()),
        EditorMessage::Head { head, .. } => match head {
            Ok(Some(head)) => blob(&head.bytes),
            Ok(None) => Ok(()),
            Err(error) => string(error),
        },
        EditorMessage::PlayAudio {
            command: AudioCommand::Load(source),
            ..
        } => {
            string(&source.media_type)?;
            blob(&source.bytes)
        }
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
        EditorMessage::WatchHostValue { key, .. } => string(key),
        EditorMessage::HostValue {
            key, value: bytes, ..
        }
        | EditorMessage::HostAction {
            key, action: bytes, ..
        } => {
            string(key)?;
            blob(bytes)
        }
        EditorMessage::WebViewCommand { command, .. } => match command {
            WebViewCommand::Open(url) | WebViewCommand::Load(url) => string(url),
            WebViewCommand::Reload | WebViewCommand::FocusApp | WebViewCommand::Close => Ok(()),
        },
        EditorMessage::WebViewEvent { event, .. } => match event {
            WebViewEvent::Navigate(value)
            | WebViewEvent::Finished(value)
            | WebViewEvent::Push(value)
            | WebViewEvent::Replace(value)
            | WebViewEvent::Title(value)
            | WebViewEvent::NewWindow(value)
            | WebViewEvent::Address(value)
            | WebViewEvent::Failed(value) => string(value),
            WebViewEvent::History(_) => Ok(()),
        },
        _ => Ok(()),
    }
}

fn validate_request(request: &HostRequest) -> Result<(), DecodeError> {
    match request {
        HostRequest::PickFile(filter) => {
            string(&filter.name)?;
            collection(filter.extensions.len())?;
            collection(filter.mime_types.len())?;
            strings(filter.extensions.iter().chain(&filter.mime_types))
        }
        HostRequest::SaveFile(file) => {
            string(&file.name)?;
            string(&file.mime_type)?;
            blob(&file.data)
        }
        HostRequest::PickBlock(filter) => block_filter(filter),
        HostRequest::ListAccess(_)
        | HostRequest::ListRevisions(_)
        | HostRequest::PreviewRevision { .. }
        | HostRequest::RestoreRevision { .. }
        | HostRequest::ReleaseRevision(_) => Ok(()),
        HostRequest::ListData => Ok(()),
        HostRequest::Fetch(url) => string(url),
        HostRequest::ReadData(path) => string(path),
        HostRequest::ExportBlock(_)
        | HostRequest::ReadPlainFile(_)
        | HostRequest::ImportBlocks(_)
        | HostRequest::ContactLink { .. } => Ok(()),
    }
}

fn validate_reply(reply: &HostReply) -> Result<(), DecodeError> {
    match reply {
        HostReply::RevisionsListed(Ok(revisions)) => collection(revisions.len()),
        HostReply::RevisionsListed(Err(error))
        | HostReply::RevisionPreviewed(Err(error))
        | HostReply::RevisionRestored(Err(error)) => string(error),
        HostReply::RevisionPreviewed(Ok(_))
        | HostReply::RevisionRestored(Ok(()))
        | HostReply::RevisionReleased => Ok(()),
        HostReply::FilePicked(Ok(Some(file))) => string(&file.name).and_then(|()| blob(&file.data)),
        HostReply::PlainFileRead(Ok(file)) => {
            string(&file.name)?;
            string(&file.mime_type)?;
            blob(&file.data)
        }
        HostReply::Fetched(FetchResult::Body(body))
        | HostReply::DataRead(FetchResult::Body(body)) => blob(body),
        HostReply::DataListed(DataListing::Files(files)) => {
            collection(files.len())?;
            strings(files)
        }
        HostReply::FilePicked(Err(message))
        | HostReply::PlainFileRead(Err(message))
        | HostReply::FileSaved(FileSave::Failed(message))
        | HostReply::BlockPicked(BlockPick::Failed(message))
        | HostReply::Fetched(FetchResult::Failed(message))
        | HostReply::DataRead(FetchResult::Failed(message))
        | HostReply::DataListed(DataListing::Failed(message))
        | HostReply::AccessListed(AccessListing::Failed(message))
        | HostReply::BlocksImported(BlockImport::Failed(message))
        | HostReply::ContactLinkMade(
            ContactLinkResult::Made(message) | ContactLinkResult::Failed(message),
        ) => string(message),
        HostReply::AccessListed(AccessListing::Listed(grants)) => {
            collection(grants.len())?;
            for grant in grants {
                string(&grant.email)?;
                string(&grant.display_name)?;
            }
            Ok(())
        }
        HostReply::FilePicked(Ok(None))
        | HostReply::FileSaved(FileSave::Saved | FileSave::Cancelled)
        | HostReply::BlockPicked(BlockPick::Chosen { .. } | BlockPick::Cancelled)
        | HostReply::BlocksImported(BlockImport::Imported { .. } | BlockImport::Cancelled) => {
            Ok(())
        }
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

fn menu(entries: &[MenuEntry]) -> Result<(), DecodeError> {
    let mut count = 0;
    menu_level(entries, 0, &mut count)
}

fn menu_level(entries: &[MenuEntry], depth: usize, count: &mut usize) -> Result<(), DecodeError> {
    if depth > MAX_MENU_DEPTH {
        return Err(DecodeError::LimitExceeded("menu depth"));
    }
    *count += entries.len();
    collection(*count)?;
    for entry in entries {
        match entry {
            MenuEntry::Command(command) => {
                strings([&command.id, &command.label, &command.glyph])?;
            }
            MenuEntry::Submenu(submenu) => {
                strings([&submenu.id, &submenu.label, &submenu.glyph])?;
                menu_level(&submenu.entries, depth + 1, count)?;
            }
            MenuEntry::Separator => {}
        }
    }
    Ok(())
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
