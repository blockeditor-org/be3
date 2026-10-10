use bincode::Options;
use serde::{Deserialize, Serialize};
use std::fmt;

mod block_ids;
mod host_value;
mod linux;
mod manifest;
mod session;
pub use block_ids::BlockIdRole;
pub use host_value::{HostAction, HostValue, decode_host, encode_host};
pub use linux::{
    Displays, HostDisplay, HostDisplayMode, HostInputDevice, HostNotification,
    HostNotificationAction, HostWindow, HostWindowId, HostWindows, Idle, IdleState, InputDevices,
    LockState, Media, MediaLevel, MediaLevels, MediaRequest, NotificationAction, Notifications,
    PlayerCommand, Power, PowerAction, PowerAvailability, ScreenLock, UnlockAttempt, WindowAction,
};
pub use manifest::{
    EditorDocument, ManifestDocument, TemplateDocument, Templates, manifest_from_json,
};
pub use session::{HostSession, QueueError, SessionFailure, SessionState};

pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_QUEUED_MESSAGES: usize = 256;
pub const MAX_CHILDREN: usize = 256;
pub const MAX_LISTED_BLOCKS: usize = 16 * 1024;
pub const REQUEST_TIMEOUT_MILLISECONDS: u64 = 5_000;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorInstanceId(pub u64);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenRequest {
    pub screen: ScreenId,
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub metrics: ViewportMetrics,
    pub frame: Option<FrameSpec>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameChrome {
    Drawn,
    #[default]
    None,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrameSpec {
    pub chrome: FrameChrome,
    pub content: Option<ChildRect>,
    pub top_bar: TopBar,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopBar {
    #[default]
    Hidden,
    Shown,
    Phone,
}

impl TopBar {
    pub fn shown(self) -> bool {
        matches!(self, Self::Shown)
    }

    pub fn phone(self) -> bool {
        matches!(self, Self::Phone)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarAction {
    Details,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuEntry {
    pub id: String,
    pub label: String,
    pub glyph: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameReport {
    pub screen: ScreenId,
    pub content: ChildRect,
    pub painted: Vec<ChildRect>,
    pub floating: Vec<ChildRect>,
    pub claims: Vec<PressClaim>,
    pub handles_back: bool,
    pub intercepted_keys: Vec<KeyChord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyChord {
    pub key: Key,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PressClaim {
    pub modifiers: Modifiers,
    pub rect: ChildRect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenSet {
    pub request_id: u64,
    pub screens: Vec<ScreenRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenPlacement {
    pub screen: ScreenId,
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub surface: u32,
    pub width: u32,
    pub height: u32,
    pub scale_factor_millis: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionSize {
    pub screen: ScreenId,
    pub logical_width: f32,
    pub logical_height: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenLayout {
    pub generation: u64,
    pub screens: Vec<ScreenPlacement>,
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
            generation: 0,
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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChildRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebViewId(pub u32);

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildContent {
    Block {
        block_id: [u8; 16],
        block_type: [u8; 16],
        view_block: Option<[u8; 16]>,
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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HostPanel {
    BlockStack,
    Performance,
    Plugins,
    Version,
}

impl HostPanel {
    pub const ALL: [Self; 4] = [
        Self::BlockStack,
        Self::Performance,
        Self::Plugins,
        Self::Version,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::BlockStack => "Block Stack State",
            Self::Performance => "Performance",
            Self::Plugins => "Plugins",
            Self::Version => "App Version",
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
    pub rect: ChildRect,
    pub clip: ChildRect,
    pub own_frame: bool,
    pub top_bar: TopBar,
    pub corner_radius: f32,
    pub layer: ChildLayer,
    pub mode: ChildMode,
    pub intrinsic: Option<Size>,
    pub rotation: f32,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Occluder {
    pub after: u32,
    pub rect: ChildRect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChildPlacements {
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub generation: u64,
    pub size: Size,
    pub children: Vec<ChildPlacement>,
    pub occluders: Vec<Occluder>,
}

impl ChildPlacements {
    pub fn occluded(&self, index: usize, x: f32, y: f32) -> bool {
        self.occluders
            .iter()
            .filter(|occluder| occluder.after as usize > index)
            .any(|occluder| occluder.rect.contains(x, y))
    }
}

impl ChildRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
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
    pub hovered: bool,
    pub active: bool,
    pub interaction: InteractionMode,
    pub capabilities: EditorCapabilities,
    pub resize: ResizeMode,
    pub error: Option<String>,
    pub menu: Vec<MenuEntry>,
    pub creation: Option<CreationProgress>,
    pub settings: Option<SettingsProgress>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsProgress {
    pub changed: bool,
    pub summary: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellDialog {
    Rename,
    Share,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessGrant {
    pub account: [u8; 16],
    pub email: String,
    pub display_name: String,
    pub administrator: bool,
    pub granted: Option<AccessLevel>,
    pub effective: AccessLevel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessListing {
    Listed(Vec<AccessGrant>),
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
    pub entry_point: String,
    pub network: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorManifest {
    pub block_type: [u8; 16],
    pub display_name: String,
    pub icon: String,
    pub templates: Vec<TemplateManifest>,
    pub children: ChildOperations,
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
    pub fn editor(&self, block_type: [u8; 16]) -> Option<&EditorManifest> {
        self.editors
            .iter()
            .find(|editor| editor.block_type == block_type)
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
    pub icon_codepoint: String,
    pub children: ChildOperations,
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
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorRegion {
    Frame,
    Preview,
    ArtifactSettings,
    Lock,
}

impl EditorRegion {
    pub const ALL: [Self; 4] = [
        Self::Frame,
        Self::Preview,
        Self::ArtifactSettings,
        Self::Lock,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    Malformed(String),
    Empty(&'static str),
    TooLong(&'static str),
    InvalidIdentity,
    InvalidBlockType,
    InvalidRegions,
    InvalidNetworkHost,
    NoEditors,
    DuplicateBlockType,
    DuplicateTemplate,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => write!(formatter, "the manifest could not be read: {error}"),
            Self::Empty(field) => write!(formatter, "{field} is empty"),
            Self::TooLong(field) => write!(formatter, "{field} is too long"),
            Self::InvalidIdentity => formatter.write_str("the plugin id is not a plain identifier"),
            Self::InvalidBlockType => formatter.write_str("the block type is not a uuid"),
            Self::InvalidRegions => {
                formatter.write_str("the regions must include the frame exactly once")
            }
            Self::InvalidNetworkHost => {
                formatter.write_str("a network host is not a plain host name")
            }
            Self::NoEditors => formatter.write_str("the plugin declares no editors"),
            Self::DuplicateBlockType => {
                formatter.write_str("two editors are declared for the same block type")
            }
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
        if self.editors.is_empty() {
            return Err(ManifestError::NoEditors);
        }
        for (index, editor) in self.editors.iter().enumerate() {
            if self.editors[..index]
                .iter()
                .any(|other| other.block_type == editor.block_type)
            {
                return Err(ManifestError::DuplicateBlockType);
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
        block_id: [u8; 16],
        block_type: [u8; 16],
        view_block: Option<[u8; 16]>,
        account_id: [u8; 16],
        workspace_id: [u8; 16],
        client_id: [u8; 16],
        editable: bool,
    },
    EditabilityChanged {
        instance: EditorInstanceId,
        editable: bool,
    },

    Content {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        content_type: [u8; 16],
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        #[serde(with = "serde_bytes")]
        session: Vec<u8>,
        applied: u64,
    },
    ContentOperations {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        operations: Vec<ContentOperation>,
    },
    Operate {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        #[serde(with = "serde_bytes")]
        operation: Vec<u8>,
    },
    WatchContent {
        instance: EditorInstanceId,
        blocks: Vec<WatchedContent>,
    },
    ResendContent {
        instance: EditorInstanceId,
        block_id: [u8; 16],
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
    ShowPresence {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        kind: [u8; 16],
        value: Option<serde_bytes::ByteBuf>,
    },
    PeerPresence {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        peers: Vec<PeerPresence>,
    },
    ViewChanged {
        instance: EditorInstanceId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        scale: f32,
    },
    ChangeView {
        instance: EditorInstanceId,
        change: ViewChange,
    },
    Present {
        instance: EditorInstanceId,
        presenting: bool,
    },
    PresentingChanged {
        instance: EditorInstanceId,
        presenting: bool,
    },
    Resized {
        instance: EditorInstanceId,
        width: f32,
        height: f32,
    },
    LeaveFrame {
        instance: EditorInstanceId,
    },
    BarAction {
        instance: EditorInstanceId,
        action: BarAction,
    },
    Menu {
        instance: EditorInstanceId,
        entries: Vec<MenuEntry>,
    },
    MenuPick {
        instance: EditorInstanceId,
        id: String,
    },
    ChildMenuPick {
        instance: EditorInstanceId,
        child: ChildId,
        id: String,
    },
    Close {
        instance: EditorInstanceId,
    },

    OpenBlock {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        block_type: [u8; 16],
        via: Option<[u8; 16]>,
    },

    ShowBlock {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        block_type: [u8; 16],
        via: Option<[u8; 16]>,
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

    Focused {
        instance: EditorInstanceId,
        block_id: Option<[u8; 16]>,
        block_type: [u8; 16],
        via: Vec<[u8; 16]>,
    },

    FocusChanged {
        instance: EditorInstanceId,
        block_id: Option<[u8; 16]>,
        block_type: [u8; 16],
        via: Vec<[u8; 16]>,
    },

    DragBlock {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        block_type: [u8; 16],
    },

    BlockCommand {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        command: BlockCommand,
    },

    DragOver {
        instance: EditorInstanceId,
        region: EditorRegion,
        x: f32,
        y: f32,
        block_id: [u8; 16],
        block_type: [u8; 16],
        dropped: bool,
    },

    DragLeft {
        instance: EditorInstanceId,
    },

    FileDrop {
        instance: EditorInstanceId,
        region: EditorRegion,
        x: f32,
        y: f32,
        files: Vec<DroppedFile>,
        dropped: bool,
    },

    FileDropLeft {
        instance: EditorInstanceId,
    },

    DragAccepted {
        instance: EditorInstanceId,
        accepted: bool,
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
        block_id: [u8; 16],
        command: AudioCommand,
    },
    AudioStatus {
        instance: EditorInstanceId,
        status: AudioStatus,
    },
    GrabCursor {
        instance: EditorInstanceId,
        grabbed: bool,
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
        block_type: [u8; 16],
        template: String,
        account_id: [u8; 16],
        workspace_id: [u8; 16],
        client_id: [u8; 16],
    },
    CreationReady {
        instance: EditorInstanceId,
        ready: bool,
    },
    CommitCreation {
        instance: EditorInstanceId,
    },
    CommitChild {
        instance: EditorInstanceId,
        child: ChildId,
        parent: BlockLocation,
        name: Option<String>,
    },
    PickRequested {
        instance: EditorInstanceId,
        pick: u64,
        filter: BlockFilter,
        parent: BlockLocation,
    },
    PickAnswered {
        instance: EditorInstanceId,
        pick: u64,
        answer: BlockPick,
    },
    CreationBlock {
        instance: EditorInstanceId,
        outcome: CreationOutcome,
    },
    OpenArtifact {
        instance: EditorInstanceId,
        source_type: [u8; 16],
        block_id: [u8; 16],
        block_type: [u8; 16],
        account_id: [u8; 16],
        workspace_id: [u8; 16],
        client_id: [u8; 16],
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    ArtifactSettings {
        instance: EditorInstanceId,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    ArtifactDescribed {
        instance: EditorInstanceId,
        description: ArtifactDescription,
    },
    ArtifactEdited {
        instance: EditorInstanceId,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    RegenerateArtifact {
        instance: EditorInstanceId,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    ArtifactRegenerated {
        instance: EditorInstanceId,
        outcome: RegenerationOutcome,
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
    Cursor {
        instance: EditorInstanceId,
        region: EditorRegion,
        cursor: CursorIcon,
    },
    Ime {
        instance: EditorInstanceId,
        region: EditorRegion,
        area: Option<ImeArea>,
    },
    Presence {
        instance: EditorInstanceId,
        visible: bool,
    },
    ReplaceChild {
        instance: EditorInstanceId,
        request_id: u64,
        old: [u8; 16],
        new: [u8; 16],
    },
    ChildReplaced {
        instance: EditorInstanceId,
        request_id: u64,
        replaced: bool,
    },
    ChildView {
        instance: EditorInstanceId,
        region: EditorRegion,
        child: ChildId,
        change: ViewChange,
    },
    ChildBar {
        instance: EditorInstanceId,
        region: EditorRegion,
        child: ChildId,
        action: BarAction,
    },
    CopyText {
        instance: EditorInstanceId,
        text: String,
    },
    PasteText {
        instance: EditorInstanceId,
    },
    AspectRatio {
        instance: EditorInstanceId,
        ratio: Option<f32>,
    },

    IntrinsicSize {
        instance: EditorInstanceId,
        size: Option<Size>,
    },
    Performance {
        instance: EditorInstanceId,
        group: String,
        measurements: Vec<PerformanceMeasurement>,
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
    CreateBlock {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        content_type: [u8; 16],
        parent: BlockLocation,
        name: Option<String>,
        artifact: Option<ArtifactSource>,
        content: Option<serde_bytes::ByteBuf>,
    },
    SetParent {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        parent: BlockLocation,
    },
    SetName {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        name: Option<String>,
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
}

impl EditorMessage {
    pub fn instance(&self) -> EditorInstanceId {
        match self {
            Self::Open { instance, .. }
            | Self::EditabilityChanged { instance, .. }
            | Self::Content { instance, .. }
            | Self::ContentOperations { instance, .. }
            | Self::Operate { instance, .. }
            | Self::WatchContent { instance, .. }
            | Self::ResendContent { instance, .. }
            | Self::SeedContent { instance, .. }
            | Self::ReplaceContent { instance, .. }
            | Self::ShowPresence { instance, .. }
            | Self::PeerPresence { instance, .. }
            | Self::ViewChanged { instance, .. }
            | Self::ChangeView { instance, .. }
            | Self::Present { instance, .. }
            | Self::PresentingChanged { instance, .. }
            | Self::Resized { instance, .. }
            | Self::LeaveFrame { instance, .. }
            | Self::BarAction { instance, .. }
            | Self::Menu { instance, .. }
            | Self::MenuPick { instance, .. }
            | Self::ChildMenuPick { instance, .. }
            | Self::Close { instance, .. }
            | Self::OpenBlock { instance, .. }
            | Self::ShowBlock { instance, .. }
            | Self::ShowPanel { instance, .. }
            | Self::WatchHostValue { instance, .. }
            | Self::HostValue { instance, .. }
            | Self::HostAction { instance, .. }
            | Self::ShowDialog { instance, .. }
            | Self::SetAccess { instance, .. }
            | Self::Focused { instance, .. }
            | Self::FocusChanged { instance, .. }
            | Self::DragBlock { instance, .. }
            | Self::BlockCommand { instance, .. }
            | Self::DragOver { instance, .. }
            | Self::DragLeft { instance, .. }
            | Self::FileDrop { instance, .. }
            | Self::FileDropLeft { instance, .. }
            | Self::DragAccepted { instance, .. }
            | Self::Request { instance, .. }
            | Self::Replied { instance, .. }
            | Self::PlayAudio { instance, .. }
            | Self::AudioStatus { instance, .. }
            | Self::GrabCursor { instance, .. }
            | Self::WebViewCommand { instance, .. }
            | Self::WebViewEvent { instance, .. }
            | Self::OpenCreation { instance, .. }
            | Self::CreationReady { instance, .. }
            | Self::CommitCreation { instance, .. }
            | Self::CommitChild { instance, .. }
            | Self::PickRequested { instance, .. }
            | Self::PickAnswered { instance, .. }
            | Self::CreationBlock { instance, .. }
            | Self::OpenArtifact { instance, .. }
            | Self::ArtifactSettings { instance, .. }
            | Self::ArtifactDescribed { instance, .. }
            | Self::ArtifactEdited { instance, .. }
            | Self::RegenerateArtifact { instance, .. }
            | Self::ArtifactRegenerated { instance, .. }
            | Self::WatchArtifacts { instance, .. }
            | Self::ArtifactStates { instance, .. }
            | Self::WatchHistory { instance, .. }
            | Self::HistoryStates { instance, .. }
            | Self::Cursor { instance, .. }
            | Self::Ime { instance, .. }
            | Self::Presence { instance, .. }
            | Self::ReplaceChild { instance, .. }
            | Self::ChildReplaced { instance, .. }
            | Self::ChildView { instance, .. }
            | Self::ChildBar { instance, .. }
            | Self::CopyText { instance, .. }
            | Self::PasteText { instance }
            | Self::AspectRatio { instance, .. }
            | Self::IntrinsicSize { instance, .. }
            | Self::Performance { instance, .. }
            | Self::WatchBlocks { instance, .. }
            | Self::Blocks { instance, .. }
            | Self::CreateBlock { instance, .. }
            | Self::SetParent { instance, .. }
            | Self::SetName { instance, .. }
            | Self::VersionControl { instance, .. }
            | Self::VersionStatus { instance, .. } => *instance,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceMeasurement {
    Duration { name: String, nanoseconds: u64 },
    Count { name: String, count: u64 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
    pub mime_types: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorIcon {
    #[default]
    Default,
    None,
    Pointer,
    Text,
    Crosshair,
    Grab,
    Grabbing,
    Move,
    NotAllowed,
    Wait,
    Progress,
    Help,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNeSw,
    ResizeNwSe,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactDescription {
    Described { source: [u8; 16], summary: String },
    Unreadable(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegenerationOutcome {
    Done,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationOutcome {
    Created([u8; 16]),
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockLocation {
    Root,
    Detached,
    Block([u8; 16]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockQuery {
    All,
    Roots,
    Detached,
    Children([u8; 16]),
    References([u8; 16]),
    Backrefs([u8; 16]),
    Parents([u8; 16]),
    Block([u8; 16]),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSource {
    pub source_type: [u8; 16],
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockInfo {
    pub block_id: [u8; 16],
    pub block_type: [u8; 16],
    pub author: [u8; 16],
    pub parent: BlockLocation,
    pub name: Option<String>,
    pub named_by_hand: bool,
    pub references: Vec<[u8; 16]>,
    pub access: AccessLevel,
    pub artifact: Option<ArtifactSource>,
    pub thumbhash: Option<Thumbhash>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thumbhash {
    pub hash: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AccessLevel {
    None,
    KnowExists,
    View,
    #[default]
    Edit,
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
pub enum ArtifactAction {
    Regenerate,
    Unlink,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactState {
    pub block_id: [u8; 16],
    pub source_type: [u8; 16],
    pub source: Option<[u8; 16]>,
    pub summary: String,
    pub error: Option<String>,
    pub regenerating: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchedContent {
    pub block_id: [u8; 16],
    pub content_type: [u8; 16],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerPresence {
    pub client: u64,
    pub kind: [u8; 16],
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentOperation {
    #[serde(with = "serde_bytes")]
    pub operation: Vec<u8>,
    pub mine: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryState {
    pub block_id: [u8; 16],
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockCommand {
    Share,
    Rename,
    Undo,
    Redo,
    Artifact {
        action: ArtifactAction,
    },
    SimulateAccess {
        access: AccessLevel,
    },
    CloseEditor,
    Unlink {
        container: [u8; 16],
    },
    Delete {
        block_type: [u8; 16],
        source: BlockLocation,
        is_reference: bool,
    },
    Move {
        block_type: [u8; 16],
        source: BlockLocation,
        destination: [u8; 16],
        is_reference: bool,
    },
    Place {
        block_type: [u8; 16],
        parent: [u8; 16],
        linked: bool,
    },
    AppMenu,
    Launcher,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionCommand {
    Adopt {
        block_id: [u8; 16],
    },
    Commit {
        message: String,
    },
    Update,
    Switch {
        branch: String,
    },
    CreateBranch {
        name: String,
    },
    NewCheckout {
        branch: String,
    },
    Resolve {
        block_id: [u8; 16],
        take: ConflictSide,
    },
    Fork,
    PullUpstream,
    PushUpstream,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictSide {
    Base,
    Ours,
    Theirs,
    Merged,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionStatus {
    pub busy: bool,
    pub error: Option<String>,
    pub behind: bool,
    pub branches: Vec<VersionBranch>,
    pub changes: Vec<VersionChange>,
    pub log: Vec<VersionCommit>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionBranch {
    pub name: String,
    pub head: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionChange {
    pub block_id: [u8; 16],
    pub block_type: [u8; 16],
    pub name: Option<String>,
    pub kind: VersionChangeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionChangeKind {
    Added,
    Removed,
    Modified,
    Moved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCommit {
    pub id: [u8; 32],
    pub parents: Vec<[u8; 32]>,
    pub author: [u8; 16],
    pub time: i64,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostRequest {
    PickFile(FileFilter),
    SaveFile(SavedFile),
    PickBlock(BlockFilter),
    ListAccess([u8; 16]),
    PasteImage,
    Fetch(String),
    ListData,
    ReadData(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostReply {
    FilePicked(FilePick),
    FileSaved(FileSave),
    BlockPicked(BlockPick),
    ImagePasted(ClipboardImage),
    Fetched(FetchResult),
    DataListed(DataListing),
    DataRead(FetchResult),
    AccessListed(AccessListing),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockFilter {
    pub name: String,
    pub block_types: Vec<[u8; 16]>,
    pub excluded: Vec<[u8; 16]>,
    pub templates: bool,
    pub place: Option<BlockLocation>,
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
pub enum FilePick {
    Chosen { name: String, data: Vec<u8> },
    Cancelled,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFile {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSave {
    Saved,
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
pub struct DroppedFile {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardImage {
    Pasted { name: String, data: Vec<u8> },
    Empty,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCommand {
    Toggle,
    Reset,
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
    Theme(Theme),
    UtcOffset(i32),
    Fonts(Fonts),
    MissingCharacters(Vec<char>),
    Screens(ScreenSet),
    Layout(ScreenLayout),
    RegionSizes(Vec<RegionSize>),
    Frames(Vec<FrameReport>),
    Input(InputBatch),
    DrawFrame { now_micros: u64 },
    FrameNeeded,
    FrameReady(FrameReady),
    Acknowledged { request_id: u64 },
    Error(ProtocolError),
    Shutdown,
    ShutdownAcknowledged,
    Editor(EditorMessage),
    BlockTypes(Catalog),
    Children(ChildPlacements),
    ChildStatuses(Vec<ChildStatus>),
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
            | Self::Theme(_)
            | Self::UtcOffset(_)
            | Self::Fonts(_)
            | Self::Screens(_)
            | Self::Input(_)
            | Self::DrawFrame { .. }
            | Self::Shutdown
            | Self::BlockTypes(_)
            | Self::ChildStatuses(_) => Direction::ToPlugin,
            Self::Hello(_)
            | Self::Acknowledged { .. }
            | Self::ShutdownAcknowledged
            | Self::Layout(_)
            | Self::MissingCharacters(_)
            | Self::RegionSizes(_)
            | Self::Frames(_)
            | Self::FrameNeeded
            | Self::FrameReady(_)
            | Self::Children(_) => Direction::ToHost,
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
            | Self::Resized { .. }
            | Self::EditabilityChanged { .. }
            | Self::Content { .. }
            | Self::ContentOperations { .. }
            | Self::PeerPresence { .. }
            | Self::ViewChanged { .. }
            | Self::PresentingChanged { .. }
            | Self::Presence { .. }
            | Self::FocusChanged { .. }
            | Self::ShowBlock { .. }
            | Self::ShowPanel { .. }
            | Self::ShowDialog { .. }
            | Self::DragOver { .. }
            | Self::DragLeft { .. }
            | Self::FileDrop { .. }
            | Self::FileDropLeft { .. }
            | Self::Replied { .. }
            | Self::AudioStatus { .. }
            | Self::WebViewEvent { .. }
            | Self::CommitCreation { .. }
            | Self::PickRequested { .. }
            | Self::ArtifactSettings { .. }
            | Self::RegenerateArtifact { .. }
            | Self::ArtifactStates { .. }
            | Self::HistoryStates { .. }
            | Self::ReplaceChild { .. }
            | Self::ChildView { .. }
            | Self::ChildBar { .. }
            | Self::Blocks { .. }
            | Self::MenuPick { .. }
            | Self::HostValue { .. }
            | Self::VersionStatus { .. } => Direction::ToPlugin,
            Self::OpenBlock { .. }
            | Self::Focused { .. }
            | Self::DragBlock { .. }
            | Self::BlockCommand { .. }
            | Self::DragAccepted { .. }
            | Self::Request { .. }
            | Self::PlayAudio { .. }
            | Self::ChangeView { .. }
            | Self::Present { .. }
            | Self::LeaveFrame { .. }
            | Self::BarAction { .. }
            | Self::Menu { .. }
            | Self::CommitChild { .. }
            | Self::SetAccess { .. }
            | Self::PickAnswered { .. }
            | Self::ChildMenuPick { .. }
            | Self::GrabCursor { .. }
            | Self::WebViewCommand { .. }
            | Self::CreationReady { .. }
            | Self::CreationBlock { .. }
            | Self::ArtifactDescribed { .. }
            | Self::ArtifactEdited { .. }
            | Self::ArtifactRegenerated { .. }
            | Self::WatchArtifacts { .. }
            | Self::WatchHistory { .. }
            | Self::Cursor { .. }
            | Self::Ime { .. }
            | Self::ChildReplaced { .. }
            | Self::CopyText { .. }
            | Self::PasteText { .. }
            | Self::AspectRatio { .. }
            | Self::IntrinsicSize { .. }
            | Self::Operate { .. }
            | Self::WatchContent { .. }
            | Self::ResendContent { .. }
            | Self::SeedContent { .. }
            | Self::ReplaceContent { .. }
            | Self::ShowPresence { .. }
            | Self::Performance { .. }
            | Self::WatchBlocks { .. }
            | Self::VersionControl { .. }
            | Self::CreateBlock { .. }
            | Self::SetParent { .. }
            | Self::WatchHostValue { .. }
            | Self::HostAction { .. }
            | Self::SetName { .. } => Direction::ToHost,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub plugin: PluginIdentity,
    pub surface: SurfaceSupport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloAccepted {
    pub host_name: String,
    pub surface: Option<SurfaceSpec>,
    pub theme: Theme,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub dark: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontRole {
    Text,
    Monospace,
    Fallback,
    Icons,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFace {
    pub role: FontRole,
    pub index: u32,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fonts {
    pub replace: bool,
    pub faces: Vec<FontFace>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginIdentity {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceSupport {
    None,
    Texture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSpec {
    pub format: SurfaceFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Bgra8UnormSrgb,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ViewChange {
    Pan {
        x: f32,
        y: f32,
    },
    Zoom {
        factor: f32,
        anchor: Option<(f32, f32)>,
    },
    Fit,
    ResumeAutoFit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewportMetrics {
    pub logical_width: f32,
    pub logical_height: f32,
    pub visible_x: f32,
    pub visible_y: f32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub scale_factor: f32,
    pub monitors: Vec<Monitor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub rect: ChildRect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputBatch {
    pub screen: ScreenId,
    pub events: Vec<InputEvent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    PointerMoved {
        x: f32,
        y: f32,
    },
    PointerLeft,
    PointerMotion {
        x: f32,
        y: f32,
    },
    PointerButton {
        button: PointerButton,
        pressed: bool,
        x: f32,
        y: f32,
    },
    Wheel {
        x: f32,
        y: f32,
        unit: WheelUnit,
    },
    WheelEnded,
    Zoom {
        factor: f32,
    },
    Touch {
        device: u64,
        finger: u64,
        phase: TouchPhase,
        x: f32,
        y: f32,
        force: Option<f32>,
    },
    Key {
        key: Key,
        pressed: bool,
        repeat: bool,
    },
    Text(String),
    Paste(String),
    Ime(ImeInput),
    Modifiers(Modifiers),
    Focus(bool),
    Back(BackPhase),
    InterceptedKey {
        key: Key,
        pressed: bool,
        repeat: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BackPhase {
    Started { edge: BackEdge },
    Progressed(f32),
    Cancelled,
    Invoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackEdge {
    None,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImeInput {
    Enabled,
    Disabled,
    SetComposingText(String),
    CommitText(String),
    FinishComposing,
    SetComposingRegion { start: u64, end: u64 },
    ReplaceText { start: u64, end: u64, text: String },
    DeleteSurrounding { before: u64, after: u64 },
    SetSelection { anchor: u64, focus: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImeText {
    pub start: u64,
    pub text: String,
    pub selection: (u64, u64),
    pub composing: Option<(u64, u64)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImeArea {
    pub rect: ChildRect,
    pub cursor: ChildRect,
    pub text: Option<ImeText>,
    pub keyboard: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WheelUnit {
    Pixels,
    Lines,
    Pages,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum Key {
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Escape,
    Tab,
    Backspace,
    Enter,
    Space,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Copy,
    Cut,
    Paste,
    Colon,
    Comma,
    Backslash,
    Slash,
    Pipe,
    Questionmark,
    Exclamationmark,
    OpenBracket,
    CloseBracket,
    OpenCurlyBracket,
    CloseCurlyBracket,
    Backtick,
    Minus,
    Period,
    Plus,
    Equals,
    Semicolon,
    Quote,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
    BrowserBack,
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MicMute,
    BrightnessUp,
    BrightnessDown,
    MediaPlayPause,
    MediaNext,
    MediaPrevious,
    MediaStop,
    Shift,
    Control,
    Alt,
    Logo,
}

impl Key {
    pub const ALL: [Self; 122] = [
        Self::ArrowDown,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::ArrowUp,
        Self::Escape,
        Self::Tab,
        Self::Backspace,
        Self::Enter,
        Self::Space,
        Self::Insert,
        Self::Delete,
        Self::Home,
        Self::End,
        Self::PageUp,
        Self::PageDown,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::Colon,
        Self::Comma,
        Self::Backslash,
        Self::Slash,
        Self::Pipe,
        Self::Questionmark,
        Self::Exclamationmark,
        Self::OpenBracket,
        Self::CloseBracket,
        Self::OpenCurlyBracket,
        Self::CloseCurlyBracket,
        Self::Backtick,
        Self::Minus,
        Self::Period,
        Self::Plus,
        Self::Equals,
        Self::Semicolon,
        Self::Quote,
        Self::Num0,
        Self::Num1,
        Self::Num2,
        Self::Num3,
        Self::Num4,
        Self::Num5,
        Self::Num6,
        Self::Num7,
        Self::Num8,
        Self::Num9,
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::H,
        Self::I,
        Self::J,
        Self::K,
        Self::L,
        Self::M,
        Self::N,
        Self::O,
        Self::P,
        Self::Q,
        Self::R,
        Self::S,
        Self::T,
        Self::U,
        Self::V,
        Self::W,
        Self::X,
        Self::Y,
        Self::Z,
        Self::F1,
        Self::F2,
        Self::F3,
        Self::F4,
        Self::F5,
        Self::F6,
        Self::F7,
        Self::F8,
        Self::F9,
        Self::F10,
        Self::F11,
        Self::F12,
        Self::F13,
        Self::F14,
        Self::F15,
        Self::F16,
        Self::F17,
        Self::F18,
        Self::F19,
        Self::F20,
        Self::F21,
        Self::F22,
        Self::F23,
        Self::F24,
        Self::F25,
        Self::F26,
        Self::F27,
        Self::F28,
        Self::F29,
        Self::F30,
        Self::F31,
        Self::F32,
        Self::F33,
        Self::F34,
        Self::F35,
        Self::BrowserBack,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::VolumeMute,
        Self::MicMute,
        Self::BrightnessUp,
        Self::BrightnessDown,
        Self::MediaPlayPause,
        Self::MediaNext,
        Self::MediaPrevious,
        Self::MediaStop,
        Self::Shift,
        Self::Control,
        Self::Alt,
        Self::Logo,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub logo: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameReady {
    pub generation: u64,
    pub repaint_after_micros: Option<u64>,
    pub presented: Option<PresentedFrame>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentedFrame {
    pub sequence: u64,
    pub damage: Vec<ScreenDamage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenDamage {
    pub screen: ScreenId,
    pub rect: SurfaceRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
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
        .map_err(|_| DecodeError::MalformedPayload)?;
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
                match event {
                    InputEvent::Text(value)
                    | InputEvent::Paste(value)
                    | InputEvent::Ime(
                        ImeInput::SetComposingText(value)
                        | ImeInput::CommitText(value)
                        | ImeInput::ReplaceText { text: value, .. },
                    ) => text(value)?,
                    _ => {}
                }
            }
            Ok(())
        }
        Message::Screens(value) => collection(value.screens.len()),
        Message::Frames(value) => {
            collection(value.len())?;
            for report in value {
                collection(report.painted.len())?;
                collection(report.floating.len())?;
                collection(report.claims.len())?;
                collection(report.intercepted_keys.len())?;
            }
            Ok(())
        }
        Message::Layout(value) => collection(value.screens.len()),
        Message::FrameReady(value) => value
            .presented
            .as_ref()
            .map_or(Ok(()), |presented| collection(presented.damage.len())),
        Message::RegionSizes(value) => collection(value.len()),
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
            Ok(())
        }
        Message::Children(value) => validate_children(value),
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
        EditorMessage::CommitChild {
            name: Some(name), ..
        } => string(name),
        EditorMessage::PickAnswered { answer, .. } => match answer {
            BlockPick::Failed(error) => string(error),
            BlockPick::Chosen { .. } | BlockPick::Cancelled => Ok(()),
        },
        EditorMessage::Performance {
            group,
            measurements,
            ..
        } => {
            string(group)?;
            collection(measurements.len())?;
            strings(measurements.iter().map(|measurement| match measurement {
                PerformanceMeasurement::Duration { name, .. }
                | PerformanceMeasurement::Count { name, .. } => name,
            }))
        }
        EditorMessage::CreationBlock { outcome, .. } => match outcome {
            CreationOutcome::Created(_) => Ok(()),
            CreationOutcome::Failed(message) => string(message),
        },
        EditorMessage::ArtifactDescribed { description, .. } => match description {
            ArtifactDescription::Described { summary, .. } => string(summary),
            ArtifactDescription::Unreadable(message) => string(message),
        },
        EditorMessage::ArtifactRegenerated { outcome, .. } => match outcome {
            RegenerationOutcome::Done => Ok(()),
            RegenerationOutcome::Failed(message) => string(message),
        },
        EditorMessage::OpenCreation { template, .. } => string(template),
        EditorMessage::Ime {
            area:
                Some(ImeArea {
                    text: Some(surrounding),
                    ..
                }),
            ..
        } => text(&surrounding.text),
        EditorMessage::OpenArtifact { data, .. }
        | EditorMessage::ArtifactSettings { data, .. }
        | EditorMessage::ArtifactEdited { data, .. }
        | EditorMessage::RegenerateArtifact { data, .. } => descriptor(data),
        EditorMessage::FileDrop { files, .. } => {
            collection(files.len())?;
            for file in files {
                string(&file.name)?;
                blob(&file.data)?;
            }
            Ok(())
        }
        EditorMessage::AudioStatus { status, .. } => match &status.error {
            Some(message) => string(message),
            None => Ok(()),
        },
        EditorMessage::Focused { via, .. } | EditorMessage::FocusChanged { via, .. } => {
            collection(via.len())
        }
        EditorMessage::WatchArtifacts { blocks, .. }
        | EditorMessage::WatchHistory { blocks, .. } => collection(blocks.len()),
        EditorMessage::HistoryStates { states, .. } => collection(states.len()),
        EditorMessage::ContentOperations { operations, .. } => collection(operations.len()),
        EditorMessage::WatchContent { blocks, .. } => collection(blocks.len()),
        EditorMessage::WatchBlocks { queries, .. } => collection(queries.len()),
        EditorMessage::Blocks { blocks, .. } => {
            if blocks.len() > MAX_LISTED_BLOCKS {
                return Err(DecodeError::LimitExceeded("block list"));
            }
            for block in blocks {
                if let Some(name) = &block.name {
                    string(name)?;
                }
                collection(block.references.len())?;
                if let Some(artifact) = &block.artifact {
                    descriptor(&artifact.data)?;
                }
            }
            Ok(())
        }
        EditorMessage::CreateBlock {
            name,
            artifact,
            content,
            ..
        } => {
            if let Some(name) = name {
                string(name)?;
            }
            if let Some(artifact) = artifact {
                descriptor(&artifact.data)?;
            }
            content.as_ref().map_or(Ok(()), |content| blob(content))
        }
        EditorMessage::SetName {
            name: Some(name), ..
        } => string(name),
        EditorMessage::VersionControl { command, .. } => match command {
            VersionCommand::Commit { message: value }
            | VersionCommand::Switch { branch: value }
            | VersionCommand::CreateBranch { name: value }
            | VersionCommand::NewCheckout { branch: value } => string(value),
            VersionCommand::Adopt { .. }
            | VersionCommand::Update
            | VersionCommand::Resolve { .. }
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
            for commit in &status.log {
                collection(commit.parents.len())?;
                string(&commit.message)?;
            }
            Ok(())
        }
        EditorMessage::ShowPresence {
            value: Some(value), ..
        } => blob(value),
        EditorMessage::PeerPresence { peers, .. } => {
            collection(peers.len())?;
            for peer in peers {
                blob(&peer.value)?;
            }
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
        EditorMessage::CopyText { text: value, .. } => text(value),
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
        EditorMessage::Menu { entries, .. } => menu(entries),
        EditorMessage::MenuPick { id, .. } | EditorMessage::ChildMenuPick { id, .. } => string(id),
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
        HostRequest::ListAccess(_) => Ok(()),
        HostRequest::PasteImage | HostRequest::ListData => Ok(()),
        HostRequest::Fetch(url) => string(url),
        HostRequest::ReadData(path) => string(path),
    }
}

fn validate_reply(reply: &HostReply) -> Result<(), DecodeError> {
    match reply {
        HostReply::FilePicked(FilePick::Chosen { name, data }) => {
            string(name).and_then(|()| blob(data))
        }
        HostReply::ImagePasted(ClipboardImage::Pasted { name, data }) => {
            string(name).and_then(|()| blob(data))
        }
        HostReply::Fetched(FetchResult::Body(body))
        | HostReply::DataRead(FetchResult::Body(body)) => blob(body),
        HostReply::DataListed(DataListing::Files(files)) => {
            collection(files.len())?;
            strings(files)
        }
        HostReply::FilePicked(FilePick::Failed(message))
        | HostReply::FileSaved(FileSave::Failed(message))
        | HostReply::BlockPicked(BlockPick::Failed(message))
        | HostReply::ImagePasted(ClipboardImage::Failed(message))
        | HostReply::Fetched(FetchResult::Failed(message))
        | HostReply::DataRead(FetchResult::Failed(message))
        | HostReply::DataListed(DataListing::Failed(message))
        | HostReply::AccessListed(AccessListing::Failed(message)) => string(message),
        HostReply::AccessListed(AccessListing::Listed(grants)) => {
            collection(grants.len())?;
            for grant in grants {
                string(&grant.email)?;
                string(&grant.display_name)?;
            }
            Ok(())
        }
        HostReply::FilePicked(FilePick::Cancelled)
        | HostReply::FileSaved(FileSave::Saved | FileSave::Cancelled)
        | HostReply::BlockPicked(BlockPick::Chosen { .. } | BlockPick::Cancelled)
        | HostReply::ImagePasted(ClipboardImage::Empty) => Ok(()),
    }
}

fn descriptor(data: &[u8]) -> Result<(), DecodeError> {
    if data.len() > MAX_OPAQUE_DESCRIPTOR_BYTES {
        Err(DecodeError::LimitExceeded("artifact settings"))
    } else {
        Ok(())
    }
}

fn menu(entries: &[MenuEntry]) -> Result<(), DecodeError> {
    collection(entries.len())?;
    for entry in entries {
        string(&entry.id)?;
        string(&entry.label)?;
        string(&entry.glyph)?;
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

pub fn paste_events(text: &str) -> Vec<InputEvent> {
    text_pieces(text, MAX_TEXT_BYTES)
        .into_iter()
        .map(InputEvent::Paste)
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
