use std::sync::Arc;

use beui::{Rect, Vec2};
use block_plugin_api::{
    ChildId, ChildLayer, ChildMode, EditorCapabilities, EditorInstanceId, EditorRegion,
    InteractionMode, PluginManifest, ResizeMode, ScreenId,
};
use uuid::Uuid;

mod audio;
mod backend;
mod calls;
mod description;
mod fonts;
pub(crate) mod graph;
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
mod wire;

pub(crate) use description::describe;
pub(crate) use input::BlockDrag;
pub(crate) use instances::EditorView;
pub(crate) use presenter::Piece;
#[cfg(target_arch = "wasm32")]
pub(crate) use presenter::shown as shown_blits;
#[cfg(target_arch = "wasm32")]
pub(crate) use runtime::place_screens;
#[cfg(target_os = "linux")]
pub(crate) use runtime::watched;
pub(crate) use runtime::{
    PACING, Pick, artifact, artifact_draft, aspect_ratio, assigned_size, block_picked, busy, close,
    commit_creation, crash, creation, creation_ready, drawing, flush, frame_child, frame_placement,
    frame_rects, hold, in_use, install, intrinsic_size, kill, menu, menu_pick, poll, present,
    presenting, preview_image, publish, reanchor, record_pacing, regenerate_artifact, region_size,
    replace_child, replace_gpu, report_child_bars, report_child_focus, report_children,
    request_painting, request_pick, resized, revoke_frame_child, running, set_artifact_states,
    set_focus, set_presence_visible, set_presented, set_shell, settle, show_block, show_dialog,
    show_panel, start_frames, take_artifact_outcome, take_artifact_watch, take_bar_actions,
    take_block_pick, take_calls, take_child_commits, take_child_menu_picks, take_crashed,
    take_created, take_focus_report, take_leaving, take_pick_answers, take_steps,
    take_view_changes, take_withdrawn_picks, warm, withdraw_pick,
};
pub(crate) use runtime::{
    RegionPlacement, RegionSlot, RegionView, back_region, drag_over_region, forward_region, frames,
    intercept_region, intercept_tap, mount_region, place_region, pressed_outside_region,
    region_damage, region_drawing, region_placed, region_published, region_view, take_block_drags,
    take_changed, take_focus_request, take_region_actions, unmount_region, unplace_region,
};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn cache_in(directory: std::path::PathBuf) {
    wasm::cache_in(directory);
}

pub(crate) fn exiting() {
    runtime::exiting();
    #[cfg(not(target_arch = "wasm32"))]
    {
        surface::stop();
        wasm::wait_for_workers(std::time::Duration::from_secs(2));
    }
}

pub(crate) const MAX_LIVE_CHILDREN: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HostChild {
    pub(crate) child: ChildId,
    pub(crate) frame_owner: bool,
    pub(crate) own_frame: bool,
    pub(crate) top_bar: block_plugin_api::TopBar,
    pub(crate) content: HostContent,
    pub(crate) rect: Rect,
    pub(crate) view_rect: Rect,
    pub(crate) occluders: Vec<Rect>,
    pub(crate) clip: Rect,
    pub(crate) layer: ChildLayer,
    pub(crate) mode: ChildMode,
    pub(crate) intrinsic: Option<Vec2>,
    pub(crate) rotation: f32,
    pub(crate) opacity: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum HostContent {
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
}

impl HostContent {
    pub(crate) fn block_id(&self) -> Option<Uuid> {
        match self {
            Self::Block { block_id, .. } => Some(*block_id),
            Self::Panel(_)
            | Self::Window(_)
            | Self::Creation { .. }
            | Self::ArtifactSettings(_) => None,
        }
    }

    pub(crate) fn editor(&self) -> Option<Uuid> {
        match self {
            Self::Block { editor, .. } => *editor,
            Self::Panel(_)
            | Self::Window(_)
            | Self::Creation { .. }
            | Self::ArtifactSettings(_) => None,
        }
    }
}

impl HostChild {
    pub(crate) fn block_id(&self) -> Option<Uuid> {
        self.content.block_id()
    }

    pub(crate) fn is_below(&self) -> bool {
        matches!(self.layer, ChildLayer::Below)
    }

    pub(crate) fn is_active(&self) -> bool {
        matches!(
            self.mode,
            ChildMode::Active | ChildMode::Live | ChildMode::InPlace
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HostChildStatus {
    pub(crate) child: ChildId,
    pub(crate) available: bool,
    pub(crate) intrinsic: Option<Vec2>,
    pub(crate) aspect_ratio: Option<f32>,
    pub(crate) active: bool,
    pub(crate) interaction: InteractionMode,
    pub(crate) capabilities: EditorCapabilities,
    pub(crate) resize: ResizeMode,
    pub(crate) error: Option<String>,
    pub(crate) menu: Vec<block_plugin_api::MenuEntry>,
    pub(crate) creation: Option<block_plugin_api::CreationProgress>,
    pub(crate) settings: Option<block_plugin_api::SettingsProgress>,
    pub(crate) preview: block_plugin_api::ChildPreview,
}

#[derive(Clone, Debug)]
pub(crate) struct ChildCommit {
    pub(crate) child: ChildId,
    pub(crate) parent: be_graph::BlockParent,
    pub(crate) name: Option<String>,
}

pub(crate) struct BlockPickRequest {
    pub(crate) op: u64,
    pub(crate) filter: block_plugin_api::BlockFilter,
}

pub(crate) struct RuntimeStatus {
    pub(crate) plugin_id: String,
    pub(crate) state: String,
    pub(crate) surface: SurfaceStatus,
    pub(crate) pass: u64,
    pub(crate) uptime: Option<std::time::Duration>,
    pub(crate) instances: Vec<InstanceStatus>,
}

pub(crate) struct SurfaceStatus {
    pub(crate) index: u32,
    pub(crate) serial: u64,
    pub(crate) pixels: u64,
    pub(crate) placements: usize,
}

pub(crate) struct InstanceStatus {
    pub(crate) instance: EditorInstanceId,
    pub(crate) block: Option<Uuid>,
    pub(crate) role: &'static str,
    pub(crate) opened: bool,
    pub(crate) aspect_ratio: Option<f32>,
    pub(crate) intrinsic: Option<Vec2>,
    pub(crate) view: Option<Rect>,
    pub(crate) artifact: Option<ArtifactStatus>,
    pub(crate) screens: Vec<ScreenStatus>,
}

pub(crate) struct ArtifactStatus {
    pub(crate) data: usize,
    pub(crate) draft: Option<usize>,
    pub(crate) description: Option<String>,
}

pub(crate) struct ScreenStatus {
    pub(crate) screen: ScreenId,
    pub(crate) region: EditorRegion,
    pub(crate) logical: Vec2,
    pub(crate) pixels: [u32; 2],
    pub(crate) scale_factor: f32,
    pub(crate) raster: f32,
    pub(crate) used: Option<Vec2>,
    pub(crate) placement: Option<[u32; 3]>,
    pub(crate) drawn: bool,
    pub(crate) children: usize,
    pub(crate) serial: u64,
}

pub(crate) fn preview_scale(size: Vec2, scale_factor: f32) -> f32 {
    const MAXIMUM: f32 = 2048.0;
    let scale = scale_factor.max(f32::EPSILON);
    let largest = size.x.max(size.y) * scale;
    match largest > MAXIMUM {
        true => MAXIMUM / largest * scale,
        false => scale,
    }
}

pub(crate) struct CreationSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<crate::editors::PluginCatalog>,
    pub(crate) client_id: Uuid,
    pub(crate) instance: EditorInstanceId,
    pub(crate) role: InstanceRole,
}

pub(crate) enum CreationState {
    Starting,
    Ready,
    Failed(String),
}

#[derive(Clone, Copy)]
pub(crate) struct EditorBlock {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) view_block: Option<Uuid>,
}

#[derive(Clone, Copy)]
pub(crate) enum InstanceRole {
    Editor(Uuid, EditorBlock),
    Creation(Uuid, &'static str),
    Artifact(Uuid, EditorBlock),
}

impl InstanceRole {
    pub(crate) fn block(self) -> Option<EditorBlock> {
        match self {
            Self::Editor(_, block) | Self::Artifact(_, block) => Some(block),
            Self::Creation(..) => None,
        }
    }
}

pub(crate) struct ArtifactSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<crate::editors::PluginCatalog>,
    pub(crate) client_id: Uuid,
    pub(crate) instance: EditorInstanceId,
    pub(crate) editor: Uuid,
    pub(crate) block: EditorBlock,
    pub(crate) data: &'a [u8],
    pub(crate) resync: bool,
}

pub(crate) enum ArtifactState {
    Starting,
    Described { source: Uuid, summary: String },
    Failed(String),
}
