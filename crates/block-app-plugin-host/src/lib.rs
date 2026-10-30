use std::sync::Arc;

use beui::{Rect, Vec2};
use block_plugin_api::{
    ChildId, ChildLayer, ChildMode, EditorCapabilities, EditorInstanceId, InteractionMode,
    ResizeMode, ScreenId, ViewId,
};
use block_plugin_manifest::PluginManifest;
use uuid::Uuid;

mod audio;
mod backend;
mod calls;
mod description;
mod docks;
mod fonts;
mod heads;
mod input;
mod instances;
mod pieces;
mod presenter;
mod previews;
mod published;
mod runtime;
mod sent;
mod session;
#[cfg(not(target_arch = "wasm32"))]
mod surface;
#[cfg(not(target_arch = "wasm32"))]
mod wasm;
#[cfg(target_arch = "wasm32")]
mod web;
mod windows;
mod wire;

pub use description::describe;
pub use input::BlockDrag;
pub use instances::EditorView;
pub use pieces::uncovered as uncovered_pieces;
pub use presenter::Piece;
pub use presenter::Pieces;
#[cfg(target_arch = "wasm32")]
pub use presenter::shown as shown_blits;
pub use presenter::{AnchoredBlits, Blit};
#[cfg(target_arch = "wasm32")]
pub use runtime::place_screens;
pub use runtime::{
    PlacedView, ViewPlace, ViewSlot, anchored_blits, anchored_drawing, back_view,
    dismiss_anchored_views, drag_over_view, forward_anchored, forward_view, frames, intercept_tap,
    intercept_view, mount_view, place_view, placed_view, pressed_outside_view, take_block_drags,
    take_changed, take_focus_request, take_view_actions, unmount_view, unplace_view, view_damage,
    view_drawing, view_placed, view_published,
};
pub use runtime::{
    artifact, artifact_draft, aspect_ratio, assigned_size, busy, card, close, commit_creation,
    content_view, crash, creation, dialog_status, dismiss, drawing, flush, forget_dialog_answer,
    frame_child, frame_rects, hold, in_use, install, intrinsic_size, kill, menu, menu_pick, picked,
    poll, present, presenting, preview_image, record_pacing, regenerate_artifact, replace_child,
    replace_gpu, report_child_bars, report_child_focus, report_children, request_painting, resized,
    revoke_frame_child, running, set_artifact_states, set_focus, set_presence_visible,
    set_presented, set_shell, settle, show_block, show_dialog, show_panel, start_frames,
    take_artifact_outcome, take_artifact_watch, take_bar_actions, take_child_commits,
    take_child_dismissals, take_child_menu_picks, take_crashed, take_created, take_focus_report,
    take_leaving, take_steps, take_view_changes, view_placement_of, view_size, warm,
};
pub use windows::{forget_windows, request_windows, take_window_requests};

#[cfg(not(target_arch = "wasm32"))]
pub fn cache_in(directory: std::path::PathBuf) {
    wasm::cache_in(directory);
}

pub fn exiting() {
    runtime::exiting();
    #[cfg(not(target_arch = "wasm32"))]
    {
        surface::stop();
        wasm::wait_for_workers(std::time::Duration::from_secs(2));
    }
}

pub const MAX_LIVE_CHILDREN: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockOpen {
    pub id: Uuid,
    pub block_type: Uuid,
    pub via: Option<Uuid>,
    pub editor: Option<Uuid>,
}

pub enum EditorAction {
    OpenBlock(BlockOpen),
    Command {
        id: Uuid,
        command: block_plugin_api::BlockCommand,
    },
    Shell {
        id: Uuid,
        command: block_plugin_api::ShellCommand,
    },
    SimulateAccess {
        id: Uuid,
        access: block_plugin_api::AccessLevel,
    },
}

#[derive(Clone, Debug, Default)]
pub struct PluginCatalog {
    pub catalog: block_plugin_api::Catalog,
    pub debug_templates: Vec<block_plugin_api::DebugTemplate>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostChild {
    pub child: ChildId,
    pub frame_owner: bool,
    pub own_frame: bool,
    pub top_bar: block_plugin_api::TopBar,
    pub content: HostContent,
    pub rect: Rect,
    pub view_rect: Rect,
    pub occluders: Vec<Rect>,
    pub clip: Rect,
    pub layer: ChildLayer,
    pub mode: ChildMode,
    pub intrinsic: Option<Vec2>,
    pub rotation: f32,
    pub opacity: f32,
    pub corner_radius: f32,
    pub dialog: Option<block_plugin_api::DialogFrame>,
    pub anchored: Option<block_plugin_api::ViewId>,
    pub keyboard: bool,
    pub sheet: bool,
    pub anchor: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostAnchored {
    pub id: block_plugin_api::ViewId,
    pub parent: Option<block_plugin_api::ViewId>,
    pub opened: u64,
    pub screen: block_plugin_api::ScreenId,
    pub rect: Rect,
    pub bounds: Rect,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub scrims: Vec<(Rect, beui::color::Color32)>,
    pub surface: u32,
    pub pixels_per_point: f32,
    pub dismiss_on_outside_press: bool,
    pub trigger: Option<Rect>,
    pub keyboard: bool,
    pub blocks_owner: bool,
    pub edge: bool,
}

impl HostAnchored {
    pub fn takes(&self, pos: beui::Pos2) -> bool {
        self.floating.iter().any(|rect| rect.contains(pos))
    }
}

pub fn dismissed_by_press(
    layers: &[HostAnchored],
    pos: beui::Pos2,
) -> Vec<block_plugin_api::ViewId> {
    let below = |root: block_plugin_api::ViewId| {
        let mut chain = vec![root];
        for layer in layers {
            if let Some(parent) = layer.parent
                && chain.contains(&parent)
                && !chain.contains(&layer.id)
            {
                chain.push(layer.id);
            }
        }
        chain
    };
    let mut dismissed = Vec::new();
    for layer in layers {
        if !layer.dismiss_on_outside_press || dismissed.contains(&layer.id) {
            continue;
        }
        let chain = below(layer.id);
        let inside = layers
            .iter()
            .filter(|held| chain.contains(&held.id))
            .any(|held| held.takes(pos) || held.trigger.is_some_and(|rect| rect.contains(pos)));
        if !inside {
            for id in chain {
                if !dismissed.contains(&id) {
                    dismissed.push(id);
                }
            }
        }
    }
    dismissed
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HostContent {
    Block {
        block_id: Uuid,
        block_type: Uuid,
        view_block: Option<Uuid>,
        editor: Option<Uuid>,
    },
    Panel(block_plugin_api::HostPanel),
    Window(block_plugin_api::HostWindowId),
    Creation {
        editor: Uuid,
        template: String,
    },
    ArtifactSettings(Uuid),
    DockTab {
        block: Uuid,
        tab: u64,
    },
    Picker {
        filter: block_plugin_api::BlockFilter,
        parent: block_plugin_api::BlockLocation,
    },
    Rename(Uuid),
}

impl HostContent {
    pub fn block_id(&self) -> Option<Uuid> {
        match self {
            Self::Block { block_id, .. } => Some(*block_id),
            Self::Panel(_)
            | Self::Window(_)
            | Self::Creation { .. }
            | Self::ArtifactSettings(_)
            | Self::DockTab { .. }
            | Self::Picker { .. }
            | Self::Rename(_) => None,
        }
    }

    pub fn editor(&self) -> Option<Uuid> {
        match self {
            Self::Block { editor, .. } => *editor,
            Self::Panel(_)
            | Self::Window(_)
            | Self::Creation { .. }
            | Self::ArtifactSettings(_)
            | Self::DockTab { .. }
            | Self::Picker { .. }
            | Self::Rename(_) => None,
        }
    }
}

impl HostChild {
    pub fn block_id(&self) -> Option<Uuid> {
        self.content.block_id()
    }

    pub fn is_below(&self) -> bool {
        matches!(self.layer, ChildLayer::Below)
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.mode,
            ChildMode::Active | ChildMode::Live | ChildMode::InPlace
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostChildStatus {
    pub child: ChildId,
    pub available: bool,
    pub intrinsic: Option<Vec2>,
    pub aspect_ratio: Option<f32>,
    pub active: bool,
    pub interaction: InteractionMode,
    pub capabilities: EditorCapabilities,
    pub resize: ResizeMode,
    pub error: Option<String>,
    pub menu: Vec<block_plugin_api::MenuEntry>,
    pub creation: Option<block_plugin_api::CreationProgress>,
    pub preview: block_plugin_api::ChildPreview,
    pub dialog: block_plugin_api::DialogStatus,
    pub picked: Option<block_plugin_api::BlockPick>,
    pub card: bool,
}

#[derive(Clone, Debug)]
pub struct ChildCommit {
    pub child: ChildId,
    pub parent: be_graph::BlockParent,
    pub name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PickerRequest {
    pub filter: block_plugin_api::BlockFilter,
    pub parent: block_plugin_api::BlockLocation,
    pub sheet: bool,
}

impl Default for PickerRequest {
    fn default() -> Self {
        Self {
            filter: block_plugin_api::BlockFilter::default(),
            parent: block_plugin_api::BlockLocation::Root,
            sheet: false,
        }
    }
}

thread_local! {
    static PICKERS: std::cell::RefCell<std::collections::HashMap<EditorInstanceId, PickerRequest>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn ask_picker(instance: EditorInstanceId, request: PickerRequest) {
    PICKERS.with(|pickers| pickers.borrow_mut().insert(instance, request));
}

pub fn forget_picker(instance: EditorInstanceId) {
    PICKERS.with(|pickers| pickers.borrow_mut().remove(&instance));
}

fn picker_request(instance: EditorInstanceId) -> Option<PickerRequest> {
    PICKERS.with(|pickers| pickers.borrow().get(&instance).cloned())
}

pub struct RuntimeStatus {
    pub plugin_id: String,
    pub state: String,
    pub surface: SurfaceStatus,
    pub steps: u64,
    pub frames: u64,
    pub uptime: Option<std::time::Duration>,
    pub instances: Vec<InstanceStatus>,
}

pub struct SurfaceStatus {
    pub index: u32,
    pub serial: u64,
    pub pixels: u64,
    pub placements: usize,
}

pub struct InstanceStatus {
    pub instance: EditorInstanceId,
    pub block: Option<Uuid>,
    pub role: &'static str,
    pub opened: bool,
    pub aspect_ratio: Option<f32>,
    pub intrinsic: Option<Vec2>,
    pub view: Option<Rect>,
    pub artifact: Option<ArtifactStatus>,
    pub screens: Vec<ScreenStatus>,
}

pub struct ArtifactStatus {
    pub data: usize,
    pub draft: Option<usize>,
    pub description: Option<String>,
}

pub struct ScreenStatus {
    pub screen: ScreenId,
    pub view: ViewId,
    pub logical: Vec2,
    pub pixels: [u32; 2],
    pub scale_factor: f32,
    pub raster: f32,
    pub used: Option<Vec2>,
    pub placement: Option<[u32; 3]>,
    pub drawn: bool,
    pub children: usize,
    pub serial: u64,
}

pub fn preview_scale(size: Vec2, scale_factor: f32) -> f32 {
    const MAXIMUM: f32 = 2048.0;
    let scale = scale_factor.max(f32::EPSILON);
    let largest = size.x.max(size.y) * scale;
    match largest > MAXIMUM {
        true => MAXIMUM / largest * scale,
        false => scale,
    }
}

pub struct CreationSlot<'a> {
    pub plugin: &'a PluginManifest,
    pub block_types: &'a Arc<crate::PluginCatalog>,
    pub client_id: Uuid,
    pub instance: EditorInstanceId,
    pub role: InstanceRole,
}

pub enum CreationState {
    Starting,
    Ready,
    Failed(String),
}

#[derive(Clone, Copy)]
pub struct EditorBlock {
    pub id: Uuid,
    pub block_type: Uuid,
    pub view_block: Option<Uuid>,
}

#[derive(Clone, Copy)]
pub enum InstanceRole {
    Editor(Uuid, EditorBlock),
    Creation(Uuid, &'static str),
    Artifact(Uuid, EditorBlock, Uuid),
    Picker(Uuid),
    Rename(Uuid, Uuid, bool),
}

impl InstanceRole {
    pub fn block(self) -> Option<EditorBlock> {
        match self {
            Self::Editor(_, block) | Self::Artifact(_, block, _) => Some(block),
            Self::Creation(..) | Self::Picker(..) | Self::Rename(..) => None,
        }
    }
}

pub struct ArtifactSlot<'a> {
    pub plugin: &'a PluginManifest,
    pub block_types: &'a Arc<crate::PluginCatalog>,
    pub client_id: Uuid,
    pub instance: EditorInstanceId,
    pub editor: Uuid,
    pub block: EditorBlock,
    pub source: Uuid,
    pub data: &'a [u8],
    pub resync: bool,
}

pub enum ArtifactState {
    Starting,
    Described(String),
    Failed(String),
}
