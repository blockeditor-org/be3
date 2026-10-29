pub use accesskit;
use serde::{Deserialize, Serialize};
pub use serde_bytes::ByteBuf;
use std::fmt;

be_ops_bindgen::generate!({
    path: "wit",
    world: "editor-plugin",
    deps: ["../ui-keys/wit", "../ui-preferences/wit", "../ui-types/wit", "../be-ops/wit", "../block-gpu-abi/wit", "../block-plugin-manifest/wit"],
    uses: {
        "be3:manifest/types@0.1.0": ::block_plugin_manifest::types,
        "be3:ui/types@0.1.0": ::ui_types::types,
        "be3:preferences/types@0.1.0": ::ui_preferences::types,
        "be3:keys/types@0.1.0": ::ui_keys::types,
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
        "view-id": crate::ViewId,
    },
    copy: [
        "uuid", "hash", "host-window-id", "editor-instance-id", "screen-id", "child-id", "web-view-id", "view-id",
    ],
    default: [
        "frame-chrome", "top-bar", "frame-spec", "size", "fonts", "screen-layout",
        "block-metadata", "artifact-state", "version-status",
        "menu-command", "menu-check", "menu-pin", "menu-heading", "block-filter", "child-preview",
        "catalog", "audio-status", "child-layer", "child-mode", "host-preference-sources",
        "host-contact", "host-image", "window-origin", "remap-state",
        "power-availability", "media-level", "audio-output", "media-levels", "wifi-security",
        "wifi-network", "wifi-link", "wifi-status", "incoming-notification", "notification-inbox",
        "notification-report", "host-program", "tray-toggle", "tray-menu-item", "host-tray-item",
        "dialog-status", "dialog-frame", "window-kind", "insets",
    ],
    inline: [
        "rename-command", "artifact-command", "unlink-command",
        "delete-command", "move-command", "place-command", "app-menu-command", "adopt-command",
        "finish-command", "branch-command", "create-branch-command", "resolve-command",
        "undo-operation-command", "performance-duration", "performance-count", "view-zoom",
        "rename-dialog",
        "block-child", "creation-child", "artifact-settings-child", "picker-child",
        "window-fullscreen", "wifi-connect", "notification-closed", "notification-action-invoked",
        "program-list", "program-launch", "program-exec", "tray-scroll", "tray-menu-chosen",
    ],
    ordered: ["child-content", "dock-tab-path", "block-filter"],
    own_debug: ["font-face"],
});

pub use accessibility::DescribedAction;
pub use be_ops as ops;
pub use blocks::{
    ArtifactAction, ArtifactState, BlockCommand, BlockInfo, BlockMetadata, BlockQuery,
    ConflictSide, StoredHead, VersionBranch, VersionChange, VersionChangeKind, VersionCommand,
    VersionCommit, VersionOperation, VersionStatus,
};
pub use debug::debug::{DebugTemplate, PerformanceMeasurement};
pub use dock::{
    DockChildRequest, DockClosedTab, DockDeclaredChild, DockHostedView, DockPickedTab, DockUpdate,
};
pub use guest::dock::DockWatch;
pub use host_services::{AccessGrant, AccessListing, ImportedBlock, Revision, ShellDialog};
pub use shared::access::AccessLevel;
pub use shell_types::ChildStatus;
pub use shell_types::{
    ArtifactDescription, AudioCommand, AudioSource, AudioStatus, BarAction, Catalog, ChildPreview,
    CreationProgress, DialogAnswer, DialogStatus, DroppedFile, EditorDescriptor, FitArea,
    PreviewImage, ShellCommand, Stopped, TemplateDescriptor, ViewChange, WebViewEvent,
};
pub use types::{
    AnchoredPlacement, AnchoredView, ChildContent, ChildLayer, ChildMode, ChildPlacement,
    ChildPlacements, DialogFrame, Failure, FontFace, FontRole, Fonts, FrameChrome, FrameCommit,
    FrameReport, FrameSpec, Hello, HelloAccepted, HostPanel, HostTransform, HostView, InputBatch,
    Insets, LayerSpace, Occluder, OpenArtifact, OpenBlock, OpenCreation, Opening, PresentedFrame,
    PressClaim, ScreenDamage, ScreenLayout, ScreenPlacement, ScreenRequest, ScreenWindow, Scrim,
    Size, SurfaceFormat, SurfaceRect, SurfaceSpec, SurfaceSupport, TimedInput, TopBar, ViewAnchor,
    ViewEdge, ViewPlacement, ViewSize, ViewportMetrics,
};
pub use types::{BlockFilter, BlockLocation, BlockPick, OpenPicker, PickedBlock};
pub use types::{
    DockBar, DockBarMove, DockBarPlace, DockBarPlacement, DockBarWindow, DockChild, DockChildClose,
    DockChildHosted, DockChildPick, DockCover, DockDeclaration, DockDeclare, DockDialog,
    DockDirection, DockDragStage, DockEdge, DockFullscreen, DockGroup, DockHome, DockHosted,
    DockItem, DockItemKind, DockLayout, DockMessage, DockOutside, DockOutsidePane,
    DockOutsidePlace, DockPane, DockPlace, DockPlacedBar, DockRequest, DockSplit, DockTab,
    DockTabPath, DockTree, DockViewRequest, DockWindow,
};
pub use types::{MenuCheck, MenuCommand, MenuHeading, MenuItem, MenuItemKind, MenuPin};
pub use types::{
    ViewRef, WindowEdge, WindowOpen, WindowOver, WindowPlacement, WindowRequest, WindowResize,
    WindowResizeBegin,
};

pub use account::HostPreferenceSources;
pub use be_ops::Watched;
pub use contacts::HostContact;
pub use desktop::{
    displays::HostDisplay,
    input::{HostInputDevice, RemapState},
    media::{AudioOutput, MediaLevel, MediaLevels, MediaRequest, PlayerCommand},
    notifications::{
        HostNotificationAction, IncomingNotification, NotificationCloseReason, NotificationInbox,
        NotificationReport, NotificationRequest, NotificationSignal, NotificationUrgency,
    },
    power::{PowerAction, PowerAvailability},
    programs::{HostProgram, ProgramAction},
    tray::{HostTrayItem, TrayMenuItem, TrayRequest, TrayStatus, TrayToggle},
    types::HostImage,
    wifi::{WifiLink, WifiNetwork, WifiRequest, WifiSecurity, WifiStatus},
    windows::{HostWindow, WindowAction, WindowKind, WindowOrigin},
};
pub use desktop_types::TrayMenuEntry;
pub use guest::account::{AdministratorWatch, LinkBaseWatch, PreferenceSourcesWatch};
pub use guest::contacts::ContactsWatch;
pub use guest::desktop::{
    displays::DisplaysWatch,
    epaper::EpaperWatch,
    input::{InputDevicesWatch, RemapsWatch},
    media::MediaWatch,
    notifications::NotificationsWatch,
    power::PowerWatch,
    programs::ProgramsWatch,
    screen_lock::ScreenLockedWatch,
    tray::TrayWatch,
    wifi::WifiWatch,
    windows::WindowsWatch,
};

mod accesskit_mirror;
mod block_ids;
mod desktop_types;
mod docking;
mod input;
mod nesting;
mod opening;
pub mod requested;
mod watch;
pub use block_ids::BlockIdRole;
use block_plugin_manifest::extension_of;
pub use block_plugin_manifest::{
    BlockTypeDescriptor, ChildOperations, EditorCapabilities, Edits, InteractionMode,
    MAX_STRING_BYTES, PanMode, ParentView, PluginIdentity, Press, ResizeMode, TemplateCategory,
};
pub use docking::{
    DockEntry, DockFloating, DockNode, DockNodes, MAX_DOCK_DEPTH, MAX_DOCK_NESTING, dock_prefix,
    dock_prefix_block,
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
pub use ui_types::{ImeArea, WindowRequests, WindowRole};
pub use watch::{Encoded, WAITING_WEB_VIEW_EVENTS, Watch};

pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_MENU_DEPTH: usize = 8;
pub const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_CHILDREN: usize = 256;
pub const MAX_ACCESSIBILITY_NODES: usize = 64 * 1024;
pub const MAX_LISTED_BLOCKS: usize = 16 * 1024;

impl PickedBlock {
    pub fn id(&self) -> uuid::Uuid {
        uuid::Uuid::from_bytes(self.block_id)
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied => formatter.write_str("The host refused this request."),
            Self::Unavailable => formatter.write_str("The host can't answer this."),
            Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Failure {}

impl From<ops::OpError> for Failure {
    fn from(error: ops::OpError) -> Self {
        match error {
            ops::OpError::Denied => Self::Denied,
            ops::OpError::Unsupported | ops::OpError::Canceled => Self::Unavailable,
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EditorInstanceId(pub u64);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ScreenId(pub u64);

impl ScreenId {
    const OPENED: u64 = 1 << 63;

    pub const fn opened(index: u64) -> Self {
        Self(Self::OPENED | index)
    }

    pub fn is_opened(self) -> bool {
        self.0 & Self::OPENED != 0
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HostWindowId(pub u64);

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
        nesting::flatten(
            entries,
            |entry| match entry {
                MenuEntry::Submenu(submenu) => &submenu.entries,
                MenuEntry::Command(_) | MenuEntry::Separator => &[],
            },
            |entry, depth| MenuItem {
                depth,
                kind: match entry {
                    MenuEntry::Command(command) => MenuItemKind::Command(command.clone()),
                    MenuEntry::Separator => MenuItemKind::Separator,
                    MenuEntry::Submenu(submenu) => MenuItemKind::Submenu(MenuHeading {
                        id: submenu.id.clone(),
                        label: submenu.label.clone(),
                        glyph: submenu.glyph.clone(),
                    }),
                },
            },
        )
    }

    pub fn nest(items: Vec<MenuItem>) -> Result<Vec<MenuEntry>, DecodeError> {
        for item in &items {
            let opens = matches!(item.kind, MenuItemKind::Submenu(_)) as usize;
            if item.depth as usize + opens > MAX_MENU_DEPTH {
                return Err(DecodeError::LimitExceeded("menu depth"));
            }
        }
        nesting::nest(
            items,
            |item| item.depth,
            |item, entries| match item.kind {
                MenuItemKind::Command(command) if entries.is_empty() => {
                    strings([&command.id, &command.label, &command.glyph])?;
                    Ok(MenuEntry::Command(command))
                }
                MenuItemKind::Separator if entries.is_empty() => Ok(MenuEntry::Separator),
                MenuItemKind::Submenu(heading) => {
                    strings([&heading.id, &heading.label, &heading.glyph])?;
                    Ok(MenuEntry::Submenu(MenuSubmenu {
                        id: heading.id,
                        label: heading.label,
                        glyph: heading.glyph,
                        entries,
                    }))
                }
                MenuItemKind::Command(_) | MenuItemKind::Separator => {
                    Err(DecodeError::MalformedPayload)
                }
            },
        )
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

    pub fn raster(&self) -> f32 {
        self.raster_millis as f32 / 1000.0
    }
}

fn millis(value: f32) -> u32 {
    (value * 1000.0).round().max(1.0) as u32
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
                    view: request.view,
                    surface: surface(request.screen),
                    width: request.metrics.pixel_width,
                    height: request.metrics.pixel_height,
                    scale_factor_millis: millis(request.metrics.scale_factor),
                    raster_millis: millis(request.metrics.raster),
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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChildId(pub u64);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WebViewId(pub u32);

#[derive(
    Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct ViewId(pub u64);

impl ViewId {
    pub const MAIN: Self = Self(0);
    const REQUESTED: u64 = 1 << 63;
    pub const PREVIEW: Self = Self(Self::REQUESTED);
    pub const ARTIFACT_SETTINGS: Self = Self(Self::REQUESTED + 1);

    pub const fn display(display: u32) -> Self {
        Self(Self::REQUESTED + 1 + display as u64)
    }

    pub fn is_main(self) -> bool {
        self == Self::MAIN
    }
}

impl ViewPlacement {
    pub fn reports(self) -> bool {
        !matches!(
            self,
            Self::Parent(ParentView::Preview | ParentView::ArtifactSettings)
        )
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct View {
    pub id: ViewId,
    pub placement: ViewPlacement,
}

impl View {
    pub const MAIN: Self = Self::parent(ParentView::Main);
    pub const PREVIEW: Self = Self::parent(ParentView::Preview);
    pub const ARTIFACT_SETTINGS: Self = Self::parent(ParentView::ArtifactSettings);

    pub const fn parent(view: ParentView) -> Self {
        Self {
            id: match view {
                ParentView::Main => ViewId::MAIN,
                ParentView::Preview => ViewId::PREVIEW,
                ParentView::ArtifactSettings => ViewId::ARTIFACT_SETTINGS,
            },
            placement: ViewPlacement::Parent(view),
        }
    }

    pub const fn display(display: u32) -> Self {
        Self {
            id: ViewId::display(display),
            placement: ViewPlacement::Host(HostView::Display(display)),
        }
    }

    pub const fn window(id: ViewId) -> Self {
        Self {
            id,
            placement: ViewPlacement::Host(HostView::Window),
        }
    }

    pub const fn dock(id: ViewId) -> Self {
        Self {
            id,
            placement: ViewPlacement::Dock,
        }
    }

    pub fn requested(id: ViewId) -> Option<Self> {
        match id {
            ViewId::MAIN => Some(Self::MAIN),
            ViewId::PREVIEW => Some(Self::PREVIEW),
            ViewId::ARTIFACT_SETTINGS => Some(Self::ARTIFACT_SETTINGS),
            ViewId(raw) => {
                let display = raw.checked_sub(ViewId::ARTIFACT_SETTINGS.0)?;
                let display = u32::try_from(display).ok().filter(|display| *display > 0)?;
                Some(Self::display(display))
            }
        }
    }

    pub fn is_main(self) -> bool {
        self.id.is_main()
    }

    pub fn is_preview(self) -> bool {
        self.placement == ViewPlacement::Parent(ParentView::Preview)
    }

    pub fn is_window(self) -> bool {
        self.placement == ViewPlacement::Host(HostView::Window)
    }
}

impl HostTransform {
    pub const IDENTITY: Self = Self {
        origin: Pos2::ZERO,
        scale: Vec2::new(1.0, 1.0),
    };

    pub fn point(self, point: Pos2) -> Pos2 {
        Pos2::new(
            self.origin.x + point.x * self.scale.x,
            self.origin.y + point.y * self.scale.y,
        )
    }

    pub fn rect(self, rect: Rect) -> Rect {
        Rect::from_min_max(self.point(rect.min), self.point(rect.max))
    }

    pub fn inverse_point(self, point: Pos2) -> Pos2 {
        Pos2::new(
            (point.x - self.origin.x) / self.scale.x,
            (point.y - self.origin.y) / self.scale.y,
        )
    }
}

impl HostPanel {
    pub const ALL: [Self; 3] = [Self::BlockStack, Self::Performance, Self::Plugins];

    pub fn title(self) -> &'static str {
        match self {
            Self::BlockStack => "Block Stack State",
            Self::Performance => "Performance",
            Self::Plugins => "Plugins",
            Self::AppMenu => "Menu",
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
            | Self::ArtifactSettings { .. }
            | Self::DockTab(_)
            | Self::Picker { .. }
            | Self::Rename(_) => None,
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

pub fn is_published(interface: &str) -> bool {
    interface == AdministratorWatch::INTERFACE
        || interface == ContactsWatch::INTERFACE
        || extension_of(interface).is_some()
}

pub use blocks::MetadataEntries;

impl Default for AccessLevel {
    fn default() -> Self {
        Self::Manage
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
        self >= Self::Edit
    }

    pub fn can_manage(self) -> bool {
        self >= Self::Manage
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "No access",
            Self::KnowExists => "Knows it exists",
            Self::View => "Can view",
            Self::Edit => "Can edit",
            Self::Manage => "Can manage",
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

impl Insets {
    pub fn between(outer: Rect, inner: Rect) -> Self {
        Self {
            left: inner.min.x - outer.min.x,
            top: inner.min.y - outer.min.y,
            right: outer.max.x - inner.max.x,
            bottom: outer.max.y - inner.max.y,
        }
    }

    pub fn inside(self, outer: Rect) -> Rect {
        Rect::from_min_max(
            Pos2::new(outer.min.x + self.left, outer.min.y + self.top),
            Pos2::new(outer.max.x - self.right, outer.max.y - self.bottom),
        )
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
            over: None,
            anchored: None,
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
        collection(self.docks.len())?;
        self.docks.iter().try_for_each(DockMessage::validate)?;
        self.reports.iter().try_for_each(FrameReport::validate)
    }
}

impl FrameReport {
    pub fn validate(&self) -> Result<(), DecodeError> {
        collection(self.anchored.len())?;
        for (index, anchored) in self.anchored.iter().enumerate() {
            let (earlier, rest) = self.anchored.split_at(index);
            if earlier.iter().any(|other| other.view == anchored.view)
                || rest
                    .iter()
                    .any(|later| later.view == anchored.placement.parent)
            {
                return Err(DecodeError::MalformedPayload);
            }
            collection(anchored.painted.len())?;
            collection(anchored.floating.len())?;
            collection(anchored.scrims.len())?;
        }
        Ok(())
    }

    pub fn anchored_parent(&self, index: usize) -> Option<ViewId> {
        let parent = self.anchored.get(index)?.placement.parent;
        self.anchored[..index]
            .iter()
            .any(|earlier| earlier.view == parent)
            .then_some(parent)
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
