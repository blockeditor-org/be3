pub use be_block;
pub use geometry;
pub use reactive;

mod content;
#[cfg(target_arch = "wasm32")]
pub mod editor_session;
mod graph;
mod host;
#[cfg(target_arch = "wasm32")]
mod panes;
#[cfg(target_arch = "wasm32")]
mod plugin;
pub mod root_settings;
#[cfg(target_arch = "wasm32")]
mod runtime;
#[cfg(target_arch = "wasm32")]
mod screens;
pub mod session;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use block_plugin_api::{
    AccessLevel, ArtifactAction, AudioStatus, BlockCommand, BlockFilter, BlockPick, ChildId,
    ChildLayer, ChildMode, ChildPlacement, ChildStatus, ClipboardImage, CursorIcon, EditorBand,
    EditorCapabilities, EditorInstanceId, EditorRegion, FetchResult, FrameChrome, FrameSpec,
    HostReply, HostRequest, InputEvent, InteractionMode, Key, Modifiers, Occluder, PointerButton,
    ResizeMode, ScreenPlacement, TouchPhase, ViewChange, WebViewCommand, WebViewEvent, WheelUnit,
};
pub use block_ui;
pub use content::ContentProjection;
pub use geometry::{Pos2, Rect, Vec2, pos2, vec2};
pub use graph::{BlockInfo, BlockList, BlockParent, BlockQuery, Blocks, GraphCommand};
pub use host::{
    Artifact, ArtifactDescription, ArtifactState, BlockDrag, BlockHistory, BlockPicker,
    BlockSource, ContentUpdate, EditorHost, FileDrop, FileFilter, FilePicker, FocusedBlock,
    HostContent, ImagePaster, OpenRequest, PastedImage, PeerPresence, PerformanceMeasurementGuard,
    PerformanceReporter, PickedBlock, PickedFile, SeededContent, ShowRequest, ShownPresence, Waker,
};
#[cfg(target_arch = "wasm32")]
pub use plugin::{Frame, Instance, PaintTarget, Plugin, Region};
#[cfg(target_arch = "wasm32")]
pub use wgpu;

#[doc(hidden)]
pub mod __private {
    #[cfg(target_arch = "wasm32")]
    pub fn initialize_tls(size: usize, align: usize) {
        crate::wasm::initialize_storage(size, align);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_wasm<P: crate::Plugin>(manifest: &str) {
        let identity = identity(manifest);
        if let Err(error) = crate::wasm::start::<P>(&identity.id, &identity.name, &identity.version)
        {
            panic!("{} could not start: {error}", identity.name);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn step_wasm() {
        if let Err(error) = crate::wasm::step() {
            panic!("the plugin could not run a frame: {error}");
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn shutdown_wasm() {
        crate::wasm::shutdown();
    }

    pub fn document(manifest: &str) -> block_plugin_api::ManifestDocument {
        block_plugin_api::ManifestDocument::parse(manifest)
            .unwrap_or_else(|error| panic!("this plugin's manifest is invalid: {error}"))
    }

    pub fn identity(manifest: &str) -> block_plugin_api::PluginIdentity {
        document(manifest).identity()
    }
}

#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! platform_entry {
    ($plugin:ty, $manifest:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_initialize_tls(size: u32, align: u32) {
            $crate::__private::initialize_tls(size as usize, align as usize);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_start() {
            $crate::__private::start_wasm::<$plugin>($manifest);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_step() {
            $crate::__private::step_wasm();
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_shutdown() {
            $crate::__private::shutdown_wasm();
        }
    };
}

#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! platform_entry {
    ($plugin:ty, $manifest:ident) => {};
}

#[macro_export]
macro_rules! plugin {
    ($plugin:ty, $manifest:expr) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!($plugin, PLUGIN_MANIFEST);
    };
}
