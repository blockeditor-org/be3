pub use be_block;
pub use geometry;

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
mod plugin;
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

pub use asked::{Asked, Task};
pub use block_plugin_api::accesskit;
pub use block_plugin_api::guest::desktop;
pub use block_plugin_api::{
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
    TrayMenuEntry, TrayMenuItem, TrayRequest, TrayStatus, TrayToggle, TrayWatch, VersionBranch,
    VersionChange, VersionChangeKind, VersionCommand, VersionCommit, VersionOperation,
    VersionStatus, ViewChange, Watched, WebViewCommand, WebViewEvent, WebViewId, WifiLink,
    WifiNetwork, WifiRequest, WifiSecurity, WifiStatus, WifiWatch, WindowAction, WindowKind,
    WindowOrigin, WindowRequests, WindowsWatch,
};
pub use block_plugin_api::{
    AnchorSpace, AnchoredPlacement, HostTransform, HostView, ParentView, Scrim, View, ViewAnchor,
    ViewEdge, ViewId, ViewPlacement, ViewRef,
};
pub use block_plugin_api::{
    HostPreferenceSources, Motion, PreferenceLayer, PreferenceSourcesWatch, Preferences, ThemeKind,
};
pub use block_type_guest::editor::{EditorOperation, FromEditor, ToEditor};
pub use block_ui;
pub use clock::{frame_time, instant_at, utc_offset, wall_clock};
pub use content::{LiveContent, Objects, Projected};
pub use editor_state::{
    ArtifactsCore, ChildState, ChildTarget, ContentCache, CreationCore, Drag, EditorCore, Fit,
    PlacedChild, PointScale, ShownEditor, SubregionContent, fit_content, short_id,
};
pub use geometry::{Pos2, Rect, Vec2, pos2, vec2};
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
pub use plugin::{Anchored, Claim, Frame, Instance, Plugin, SettingsDraft, ShownView, ViewHandler};
#[cfg(target_arch = "wasm32")]
pub use plugin::{PaintTarget, SurfaceGpu, surface_gpu};
pub use preferences::host_preferences;
#[cfg(target_arch = "wasm32")]
pub use previews::preview_texture;
pub use state::{Subscription, Unready};
#[cfg(target_arch = "wasm32")]
pub use wgpu;

#[doc(hidden)]
pub mod __private {
    #[cfg(target_arch = "wasm32")]
    pub use block_guest_abi::{Flow, Guest, Waker, guest};
    #[cfg(target_arch = "wasm32")]
    pub use block_plugin_api::export_editor_plugin;

    #[cfg(target_arch = "wasm32")]
    pub use crate::wasm::Editor;

    #[cfg(target_arch = "wasm32")]
    pub struct App {
        name: &'static str,
        open: crate::screens::Opener,
    }

    #[cfg(target_arch = "wasm32")]
    pub fn app<P: crate::Plugin>(name: &'static str) -> App {
        App {
            name,
            open: P::open,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_wasm(
        manifest: &str,
        apps: Vec<App>,
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
        let mut implemented: Vec<_> = apps.iter().map(|app| app.name).collect();
        declared.sort_unstable();
        implemented.sort_unstable();
        assert_eq!(
            declared, implemented,
            "{name} must implement exactly the editors its manifest declares"
        );
        let apps = apps
            .into_iter()
            .map(|app| {
                let editor = manifest
                    .editors
                    .iter()
                    .find(|editor| editor.name == app.name)
                    .expect("every app names a declared editor");
                (uuid::Uuid::from_bytes(editor.id), app.open)
            })
            .collect();
        let identity = manifest.identity;
        let started = crate::wasm::Started {
            apps,
            id: identity.id,
            name: identity.name,
            version: identity.version,
        };
        Editor::start(started, waker, outbound)
    }

    pub fn parsed(manifest: &str) -> block_plugin_manifest::PluginManifest {
        block_plugin_manifest::manifest_from_json(manifest)
            .unwrap_or_else(|error| panic!("this plugin's manifest is invalid: {error}"))
    }
}

#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! platform_entry {
    ($manifest:ident, $apps:expr) => {
        struct PluginGuest($crate::__private::Editor);

        impl $crate::__private::Guest for PluginGuest {
            fn start(
                waker: $crate::__private::Waker,
                outbound: &mut Vec<Vec<u8>>,
            ) -> Result<Self, String> {
                $crate::__private::start_wasm($manifest, $apps, waker, outbound).map(PluginGuest)
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

        $crate::__private::export_editor_plugin!();
        $crate::__private::guest!(PluginGuest, __register_editor_plugin);
    };
}

#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! platform_entry {
    ($manifest:ident, $apps:expr) => {};
}

#[macro_export]
macro_rules! plugin {
    ($manifest:expr, { $($name:ident => $plugin:ty),+ $(,)? }) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!(
            PLUGIN_MANIFEST,
            vec![$(
                $crate::__private::app::<$plugin>(stringify!($name))
            ),+]
        );
    };
}
