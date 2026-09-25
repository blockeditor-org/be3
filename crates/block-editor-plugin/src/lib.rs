pub use be_block;
pub use beui;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub mod beui_frame;
mod block_link;
mod child;
mod chrome;
mod content;
pub mod database;
mod datetime;
mod editor;
#[cfg(target_arch = "wasm32")]
mod editor_session;
mod file_chooser;
mod graph;
mod host;
#[cfg(target_arch = "wasm32")]
mod panes;
mod related_content;
pub mod root_settings;
#[cfg(target_arch = "wasm32")]
mod runtime;
#[cfg(target_arch = "wasm32")]
mod screens;
pub mod session;
pub mod version_control;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use block_link::{BlockDisplay, BlockLink, watch_block_label};
pub use block_plugin_api::{
    AccessLevel, ArtifactAction, AudioStatus, BlockCommand, BlockFilter, BlockPick, ChildId,
    ChildLayer, ChildMode, ChildPlacement, ChildStatus, ClipboardImage, ConflictSide, EditorBand,
    EditorCapabilities, EditorInstanceId, EditorRegion, FetchResult, HostReply, HostRequest,
    InteractionMode, Occluder, ResizeMode, VersionBranch, VersionChange, VersionChangeKind,
    VersionCommand, VersionCommit, VersionStatus, ViewChange, WebViewCommand, WebViewEvent,
};
pub use block_ui;
pub use child::{ChildBlock, ChildHandle as ChildBlockHandle};
pub use chrome::{SIDEBAR_WIDTH, Side, Sidebar, Toolbar};
pub use content::ContentProjection;
pub use datetime::DateTimeRow;
pub use editor::{Artifacts, ChildState, ChildTarget, Creation, Drag, Editor, fit_content};
pub use file_chooser::{FileChooser, content_file_creation};
pub use graph::{BlockInfo, BlockList, BlockParent, BlockQuery, Blocks, GraphCommand};
pub use host::{
    Artifact, ArtifactDescription, ArtifactState, BeuiView, BlockDrag, BlockHistory, BlockPicker,
    BlockSource, ContentUpdate, EditorHost, FileDrop, FileFilter, FilePicker, FocusedBlock,
    HostContent, ImagePaster, OpenRequest, PastedImage, PeerPresence, PerformanceMeasurementGuard,
    PerformanceReporter, PickedBlock, PickedFile, SeededContent, ShowRequest, ShownPresence, Waker,
};
pub use related_content::RelatedContent;
pub use version_control::{VersionHistory, short_id};

pub trait BeuiApp: 'static {
    fn view(editor: Editor) -> beui::NodeId;
    fn preview_view(_editor: Editor) -> beui::NodeId {
        beui::reactive::Frame().build()
    }
    fn creation_view(_creation: Creation) -> beui::NodeId {
        beui::reactive::Frame().build()
    }
    fn create_block(creation: &Creation) -> Result<uuid::Uuid, String> {
        creation.create_block()
    }
    fn connect_artifact(_artifacts: &Artifacts) {}
    fn describe_artifact(_data: &[u8]) -> Result<ArtifactDescription, String> {
        Err("this editor does not generate artifacts".into())
    }
    fn artifact_settings_view(_artifacts: Artifacts) -> beui::NodeId {
        beui::reactive::Frame().build()
    }
    fn intrinsic_size() -> Option<beui::Vec2> {
        None
    }
    fn aspect_ratio() -> Option<f32> {
        None
    }
}

#[doc(hidden)]
pub mod __private {
    #[cfg(target_arch = "wasm32")]
    pub fn initialize_tls(size: usize, align: usize) {
        crate::wasm::initialize_storage(size, align);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_wasm<A: crate::BeuiApp>(manifest: &str) {
        let identity = identity(manifest);
        if let Err(error) = crate::wasm::start::<A>(&identity.id, &identity.name, &identity.version)
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
    ($app:ty, $manifest:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_initialize_tls(size: u32, align: u32) {
            $crate::__private::initialize_tls(size as usize, align as usize);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_start() {
            $crate::__private::start_wasm::<$app>($manifest);
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
    ($app:ty, $manifest:ident) => {};
}

#[macro_export]
macro_rules! beui_plugin {
    ($app:ty, $manifest:expr) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!($app, PLUGIN_MANIFEST);
    };
}
