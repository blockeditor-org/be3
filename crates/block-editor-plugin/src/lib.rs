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
    PerformanceReporter, PickedBlock, PickedFile, Pushed, SeededContent, ShowRequest,
    ShownPresence, Waker,
};
#[cfg(target_arch = "wasm32")]
pub use plugin::{Frame, Ime, Instance, PaintTarget, Plugin, Region};
#[cfg(target_arch = "wasm32")]
pub use wgpu;

#[doc(hidden)]
pub mod __private {
    #[cfg(target_arch = "wasm32")]
    pub struct App {
        block_type: uuid::Uuid,
        open: crate::screens::Opener,
    }

    #[cfg(target_arch = "wasm32")]
    pub fn app<P: crate::Plugin>(block_type: uuid::Uuid) -> App {
        App {
            block_type,
            open: P::open,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn only_app<P: crate::Plugin>(manifest: &str) -> Vec<App> {
        let editors = document(manifest).editors;
        let [editor] = editors.as_slice() else {
            panic!(
                "a plugin with one app declares exactly one editor, not {}",
                editors.len()
            );
        };
        let block_type = uuid::Uuid::parse_str(&editor.block_type)
            .unwrap_or_else(|error| panic!("this plugin's block type is invalid: {error}"));
        vec![app::<P>(block_type)]
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
macro_rules! plugin {
    ($plugin:ty, $manifest:expr) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!(
            PLUGIN_MANIFEST,
            $crate::__private::only_app::<$plugin>(PLUGIN_MANIFEST)
        );
    };
    ($manifest:expr, { $($content:ty => $plugin:ty),+ $(,)? }) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!(
            PLUGIN_MANIFEST,
            vec![$(
                $crate::__private::app::<$plugin>(
                    <$content as $crate::be_block::BlockContent>::CONTENT_TYPE,
                )
            ),+]
        );
    };
}
