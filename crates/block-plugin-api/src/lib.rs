pub use accesskit;
use serde::{Deserialize, Serialize};
pub use serde_bytes::ByteBuf;
use std::fmt;

be_ops_bindgen::generate!({
    path: "wit",
    world: "editor-plugin",
    deps: ["../ui-types/wit", "../be-ops/wit", "../block-gpu-abi/wit"],
    uses: {
        "be3:ui/types@0.1.0": ::ui_types::types,
        "be3:gpu/device@0.1.0": ::block_gpu_abi::device,
    },
    local: ["be3:gpu/device@0.1.0"],
    with: {
        "uuid": [u8; 16],
        "hash": [u8; 32],
        "editor-instance-id": crate::EditorInstanceId,
        "screen-id": crate::ScreenId,
        "host-window-id": crate::HostWindowId,
        "child-id": crate::ChildId,
        "web-view-id": crate::WebViewId,
        "rgb": [u8; 3],
    },
    copy: [
        "uuid", "hash", "host-window-id", "editor-instance-id", "screen-id", "child-id", "web-view-id", "rgb",
    ],
    default: [
        "frame-chrome", "top-bar", "frame-spec", "size", "fonts", "screen-layout",
        "block-metadata", "artifact-state", "version-status",
        "menu-command", "menu-check", "menu-pin", "menu-heading", "block-filter", "child-preview",
        "catalog", "audio-status", "child-layer", "child-mode", "interaction-mode", "pan-mode",
        "editor-capabilities", "resize-mode", "child-operations", "press", "template-category",
    ],
    inline: [
        "rename-command", "artifact-command", "simulate-access-command", "unlink-command",
        "delete-command", "move-command", "place-command", "app-menu-command", "adopt-command",
        "finish-command", "branch-command", "create-branch-command", "resolve-command",
        "undo-operation-command", "performance-duration", "performance-count", "view-zoom",
        "artifact-described", "rename-dialog", "creation-options", "chosen-block",
        "block-child", "creation-child", "artifact-settings-child",
    ],
    lenient: ["editor-capabilities", "child-operations"],
    ordered: ["child-content"],
    own_debug: ["font-face"],
});

pub use accessibility::DescribedAction;
pub use be_ops as ops;
pub use access::AccessLevel;
pub use blocks::{
    ArtifactAction, ArtifactState, BlockCommand, BlockInfo, BlockLocation,
    BlockMetadata, BlockQuery, ConflictSide, StoredHead, VersionBranch, VersionChange,
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
pub use shell_types::{
    BlockTypeDescriptor, ChildOperations, ChildStatus, EditorCapabilities, Edits, InteractionMode,
    PanMode, Press, ResizeMode, TemplateCategory,
};
pub use types::{
    ChildContent, ChildLayer, ChildMode, ChildPlacement, ChildPlacements, EditorRegion, FontFace,
    FontRole, Fonts, FrameChrome, FrameCommit, FrameReport, FrameSpec, Hello, HelloAccepted,
    HostPanel, InputBatch, Monitor, Occluder, OpenArtifact, OpenBlock, OpenCreation, Opening,
    PluginIdentity, PresentedFrame, PressClaim, RegionSize, ScreenDamage, ScreenLayout,
    ScreenPlacement, ScreenRequest, ScreenWindow, Size, SurfaceFormat, SurfaceRect, SurfaceSpec,
    SurfaceSupport, TimedInput, TopBar, ViewportMetrics,
};

mod accesskit_mirror;
mod block_ids;
mod host_value;
mod input;
mod linux;
mod manifest;
mod nesting;
mod opening;
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
pub use watch::{Encoded, Watch};

pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_MENU_DEPTH: usize = 8;
pub const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_CHILDREN: usize = 256;
pub const MAX_ACCESSIBILITY_NODES: usize = 64 * 1024;
pub const MAX_LISTED_BLOCKS: usize = 16 * 1024;

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Description {
    pub tree: Option<accesskit::TreeUpdate>,
    pub actions: Vec<DescribedAction>,
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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WebViewId(pub u32);

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

impl ChildPlacements {
    pub fn occluded(&self, index: usize, point: Pos2) -> bool {
        self.occluders
            .iter()
            .filter(|occluder| occluder.after as usize > index)
            .any(|occluder| occluder.rect.contains_half_open(point))
    }
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

pub use blocks::MetadataEntries;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebViewCommand {
    Open(String),
    Load(String),
    Reload,
    FocusApp,
    Close,
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

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    MalformedPayload,
    LimitExceeded(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DecodeError {}

impl FrameCommit {
    pub fn validate(&self) -> Result<(), DecodeError> {
        for placements in &self.children {
            let mut covered = 0;
            for occluder in &placements.occluders {
                let after = occluder.after as usize;
                if after > placements.children.len() || after < covered {
                    return Err(DecodeError::MalformedPayload);
                }
                covered = after;
            }
        }
        Ok(())
    }
}

impl embed::blocks::Call {
    pub fn validate(&self) -> Result<(), DecodeError> {
        use embed::blocks::Call;
        match self {
            Call::CreateBlock {
                defined, content, ..
            } => {
                metadata(defined)?;
                content.as_ref().map_or(Ok(()), |content| blob(content))
            }
            Call::Define {
                value: Some(value), ..
            } => descriptor(value),
            Call::SeedContent { bytes, .. } | Call::ReplaceContent { bytes, .. } => blob(bytes),
            Call::ReplicaSend { frame, .. } => blob(frame),
            Call::Version { command, .. } => match command {
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
            _ => Ok(()),
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
    entries.iter().try_for_each(|(_, value)| descriptor(value))
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

fn strings<'a>(values: impl IntoIterator<Item = &'a String>) -> Result<(), DecodeError> {
    for value in values {
        string(value)?;
    }
    Ok(())
}

fn blob(value: &[u8]) -> Result<(), DecodeError> {
    if value.len() > MAX_BLOB_BYTES {
        Err(DecodeError::LimitExceeded("blob"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
