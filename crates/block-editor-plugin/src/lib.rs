pub use be_block;
pub use beui;

pub mod beui_frame;
mod block_link;
mod child;
mod chrome;
mod content;
pub mod database;
mod datetime;
mod editor;
mod editor_session;
mod file_chooser;
mod graph;
pub mod headless;
mod host;
#[cfg(target_arch = "wasm32")]
mod panes;
mod related_content;
pub mod root_settings;
#[cfg(target_arch = "wasm32")]
mod runtime;
mod screens;
pub mod session;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use block_link::{BlockDisplay, BlockLink, watch_block_label};
pub use block_plugin_api::{
    AccessLevel, ArtifactAction, AudioStatus, BlockCommand, BlockFilter, BlockPick, ChildId,
    ChildLayer, ChildMode, ChildPlacement, ChildStatus, ClipboardImage, EditorBand,
    EditorCapabilities, EditorInstanceId, EditorRegion, FetchResult, HostReply, HostRequest,
    InteractionMode, Occluder, ResizeMode, ViewChange, WebViewCommand, WebViewEvent,
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
    PerformanceReporter, PickedBlock, PickedFile, Pushed, SeededContent, ShowRequest,
    ShownPresence, Waker,
};
pub use related_content::RelatedContent;

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
    pub struct App {
        block_type: uuid::Uuid,
        open: crate::screens::Opener,
    }

    #[cfg(target_arch = "wasm32")]
    pub fn app<A: crate::BeuiApp>(block_type: uuid::Uuid) -> App {
        App {
            block_type,
            open: crate::editor_session::EditorSession::new::<A>,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn only_app<A: crate::BeuiApp>(manifest: &str) -> Vec<App> {
        let editors = document(manifest).editors;
        let [editor] = editors.as_slice() else {
            panic!(
                "a plugin with one app declares exactly one editor, not {}",
                editors.len()
            );
        };
        let block_type = uuid::Uuid::parse_str(&editor.block_type)
            .unwrap_or_else(|error| panic!("this plugin's block type is invalid: {error}"));
        vec![app::<A>(block_type)]
    }

    pub fn declared_block_types(manifest: &str) -> Vec<uuid::Uuid> {
        document(manifest)
            .editors
            .iter()
            .map(|editor| {
                uuid::Uuid::parse_str(&editor.block_type)
                    .unwrap_or_else(|error| panic!("this plugin's block type is invalid: {error}"))
            })
            .collect()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn initialize_tls(size: usize, align: usize) {
        crate::wasm::initialize_storage(size, align);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_wasm(manifest: &str, apps: Vec<App>) {
        let identity = identity(manifest);
        let mut declared = declared_block_types(manifest);
        let mut implemented: Vec<_> = apps.iter().map(|app| app.block_type).collect();
        declared.sort();
        implemented.sort();
        assert_eq!(
            declared, implemented,
            "{} must implement exactly the editors its manifest declares",
            identity.name
        );
        let apps = apps
            .into_iter()
            .map(|app| (app.block_type, app.open))
            .collect();
        if let Err(error) =
            crate::wasm::start(apps, &identity.id, &identity.name, &identity.version)
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
    ($manifest:ident, $apps:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_initialize_tls(size: u32, align: u32) {
            $crate::__private::initialize_tls(size as usize, align as usize);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_start() {
            $crate::__private::start_wasm($manifest, $apps);
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
    ($manifest:ident, $apps:expr) => {};
}

#[macro_export]
macro_rules! beui_plugin {
    ($app:ty, $manifest:expr) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!(
            PLUGIN_MANIFEST,
            $crate::__private::only_app::<$app>(PLUGIN_MANIFEST)
        );
    };
    ($manifest:expr, { $($content:ty => $app:ty),+ $(,)? }) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!(
            PLUGIN_MANIFEST,
            vec![$(
                $crate::__private::app::<$app>(
                    <$content as $crate::be_block::BlockContent>::CONTENT_TYPE,
                )
            ),+]
        );
    };
}
