pub use be_block;
pub use be_lib_geometry as geometry;

mod app;
mod asked;
mod clock;
mod content;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod cooldown;
mod docks;
pub mod driver;
pub mod editor_session;
mod editor_state;
pub mod fonts;
mod graph;
pub mod headless;
mod host;
mod layered_settings;
mod link;
mod module;
#[cfg(target_arch = "wasm32")]
mod panes;
mod preferences;
#[cfg(target_arch = "wasm32")]
mod previews;
pub mod root_settings;
#[cfg(target_arch = "wasm32")]
mod runtime;
mod screens;
pub mod session;
mod state;
#[cfg(target_arch = "wasm32")]
mod wasm;
mod wire;

pub use app::{
    Anchored, App, Claim, Frame, Instance, SettingsDraft, ShownView, ViewHandler, fitted_pixels,
};
#[cfg(target_arch = "wasm32")]
pub use app::{PaintTarget, SurfaceGpu, surface_gpu};
pub use asked::{Asked, Task};
pub use be_lib_geometry::{Pos2, Rect, Vec2, pos2, vec2};
pub use be_sdk_api::accesskit;
pub use be_sdk_api::guest::desktop;
pub use be_sdk_api::{
    AccessGrant, AccessLevel, AccessListing, AdministratorWatch, ArtifactAction, AudioCommand,
    AudioOutput, AudioSource, AudioStatus, BarAction, BlockCommand, BlockFilter, BlockLocation,
    BlockPick, BlockTypeDescriptor, Catalog, ChildContent, ChildId, ChildLayer, ChildMode,
    ChildOperations, ChildPlacement, ChildPreview, ChildStatus, ConflictSide, ContactsWatch,
    CreationProgress, CursorIcon, DescribedAction, Description, DialogAnswer, DialogFrame,
    DialogStatus, DisplayMode, DisplaysWatch, EditorCapabilities, EditorDescriptor,
    EditorInstanceId, Edits, EpaperWatch, Failure, FileSave, FrameChrome, FrameSpec, HostContact,
    HostDisplay, HostImage, HostInputDevice, HostNotificationAction, HostPanel, HostProgram,
    HostTrayItem, HostWindow, HostWindowId, ImeArea, ImportedBlock, IncomingNotification, Input,
    InputDevicesWatch, InteractionMode, Key, KeyChord, LinkBaseWatch, MediaLevel, MediaLevels,
    MediaRequest, MediaWatch, MenuCheck, MenuCommand, MenuEntry, MenuPin, MenuSubmenu, Modifiers,
    NotificationCloseReason, NotificationInbox, NotificationReport, NotificationRequest,
    NotificationSignal, NotificationUrgency, NotificationsWatch, Occluder, PenPhase, PickedBlock,
    PlayerCommand, PointerButton, PowerAction, PowerAvailability, PowerWatch, Press, PreviewImage,
    ProgramAction, ProgramCatalog, ProgramsWatch, RemapState, RemapsWatch, ResizeMode, Revision,
    Rotation, ScreenLockedWatch, ScreenPlacement, ScrollDelta, ShellCommand, ShellDialog, Size,
    Stopped, SurfaceRect, TemplateCategory, TemplateDescriptor, TimedInput, TopBar, TouchPhase,
    TrayClick, TrayMenuEntry, TrayMenuItem, TrayRequest, TrayStatus, TrayToggle, TrayWatch, VersionBranch,
    VersionChange, VersionChangeKind, VersionCommand, VersionCommit, VersionOperation,
    VersionStatus, ViewChange, Watched, WebViewCommand, WebViewEvent, WebViewId, WifiLink,
    WifiNetwork, WifiRequest, WifiSecurity, WifiStatus, WifiWatch, WindowAction, WindowKind,
    WindowMenu, WindowOrigin, WindowRequests, WindowsWatch,
};
pub use be_sdk_api::{
    AnchorSpace, AnchoredPlacement, HostTransform, HostView, ParentView, Scrim, View, ViewAnchor,
    ViewEdge, ViewId, ViewPlacement, ViewRef,
};
pub use be_sdk_api::{
    HostPreferenceSources, Motion, PreferenceLayer, PreferenceSourcesWatch, Preferences, ThemeKind,
};
pub use be_sdk_type_guest::editor::{EditorOperation, FromEditor, ToEditor};
pub use be_sdk_ui;
pub use clock::{frame_time, instant_at, utc_offset, wall_clock};
pub use content::{LiveContent, Objects, Projected};
pub use editor_state::{
    ArtifactsCore, ChildState, ChildTarget, ContentCache, CreationCore, Drag, EditorCore, Fit,
    PlacedChild, PointScale, ShownEditor, SubregionContent, fit_content, short_id,
};
pub use graph::{BlockGraph, BlockInfo, BlockParent, BlockQuery, BlockWatch, GraphCommand};
pub use host::{
    Artifact, ArtifactState, BlockDrag, BlockHistory, EditorHost, FileDrop, FileFilter,
    FocusedBlock, HeadRead, HostContent, OpenRequest, PeerMessage, PeerPresence,
    PerformanceMeasurementGuard, PerformanceReporter, PickedFile, PickerRequest, Pushed,
    RenameRequest, SavedFile, SeededContent, SentToPeers, ShowRequest, ShownPresence, StoredHead,
    Waker,
};
pub use layered_settings::{SettingsAnswer, SettingsChangeLog, SettingsLayers, SettingsTarget};
pub use module::Module;
pub use preferences::host_preferences;
#[cfg(target_arch = "wasm32")]
pub use previews::preview_texture;
pub use state::{Subscription, Unready};
#[cfg(target_arch = "wasm32")]
pub use wgpu;

#[doc(hidden)]
pub mod __private {
    #[cfg(target_arch = "wasm32")]
    pub use be_wasm_guest_abi::{Flow, Guest, Waker, guest};

    #[cfg(target_arch = "wasm32")]
    pub use crate::wasm::Editor;

    #[cfg(target_arch = "wasm32")]
    pub struct EditorEntry {
        name: &'static str,
        open: crate::screens::Opener,
    }

    #[cfg(target_arch = "wasm32")]
    pub fn editor_entry<P: crate::App>(name: &'static str) -> EditorEntry {
        EditorEntry {
            name,
            open: P::open,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_wasm(
        manifest: &str,
        entries: Vec<EditorEntry>,
        waker: Waker,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Editor, String> {
        let manifest = parsed(manifest);
        let name = &manifest.identity.name;
        let mut declared: Vec<_> = manifest
            .editors
            .iter()
            .map(|editor| editor.name.as_str())
            .collect();
        let mut implemented: Vec<_> = entries.iter().map(|entry| entry.name).collect();
        declared.sort_unstable();
        implemented.sort_unstable();
        assert_eq!(
            declared, implemented,
            "{name} must implement exactly the editors its manifest declares"
        );
        let openers = entries
            .into_iter()
            .map(|entry| {
                let editor = manifest
                    .editors
                    .iter()
                    .find(|editor| editor.name == entry.name)
                    .expect("every entry names a declared editor");
                (uuid::Uuid::from_bytes(editor.id), entry.open)
            })
            .collect();
        let identity = manifest.identity;
        let started = crate::wasm::Started {
            openers,
            id: identity.id,
            name: identity.name,
            version: identity.version,
        };
        Editor::start(started, waker, outbound)
    }

    pub fn parsed(manifest: &str) -> be_sdk_manifest::AppManifest {
        be_sdk_manifest::manifest_from_json(manifest)
            .unwrap_or_else(|error| panic!("this app's manifest is invalid: {error}"))
    }
}

#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! platform_entry {
    ($manifest:ident, $entries:expr) => {
        struct AppGuest($crate::__private::Editor);

        impl $crate::__private::Guest for AppGuest {
            fn start(
                waker: $crate::__private::Waker,
                outbound: &mut Vec<Vec<u8>>,
            ) -> Result<Self, String> {
                $crate::__private::start_wasm($manifest, $entries, waker, outbound).map(AppGuest)
            }

            fn step(
                &mut self,
                inbound: Vec<Vec<u8>>,
                woken: bool,
                outbound: &mut Vec<Vec<u8>>,
            ) -> Result<$crate::__private::Flow, String> {
                self.0.step(inbound, woken, outbound)
            }
        }

        $crate::__private::guest!(AppGuest);
    };
}

#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! platform_entry {
    ($manifest:ident, $entries:expr) => {};
}

#[macro_export]
macro_rules! app {
    ($manifest:expr, { $($name:ident => $app:ty),+ $(,)? }) => {
        const APP_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!(
            APP_MANIFEST,
            vec![$(
                $crate::__private::editor_entry::<$app>(stringify!($name))
            ),+]
        );
    };
}
