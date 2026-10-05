use std::sync::Arc;

use beui::{Rect, Vec2, vec2};
use block_plugin_api::{
    ChildId, ChildLayer, ChildMode, EditorCapabilities, EditorInstanceId, EditorRegion,
    InteractionMode, PluginManifest, ResizeMode, ScreenId,
};
use uuid::Uuid;

mod audio;
mod backend;
mod clipboard;
mod fonts;
pub(crate) mod graph;
mod input;
mod instances;
mod pieces;
mod presenter;
mod runtime;
#[cfg(not(target_arch = "wasm32"))]
mod surface;
#[cfg(not(target_arch = "wasm32"))]
mod wasm;
#[cfg(target_arch = "wasm32")]
mod web;
mod web_view;

pub(crate) use instances::EditorView;
pub(crate) use presenter::Piece;
#[cfg(target_arch = "wasm32")]
pub(crate) use presenter::shown as shown_blits;
#[cfg(target_arch = "wasm32")]
pub(crate) use runtime::place_screens;
pub(crate) use runtime::{
    PACING, artifact, artifact_draft, aspect_ratio, block_picked, close, commit_creation, creation,
    creation_ready, flush, frame_child, frame_rects, hold, install, intrinsic_size, kill, menu,
    menu_pick, poll, present, presenting, record_pacing, regenerate_artifact, region_size,
    replace_child, report_child_bars, report_child_views, report_children, request_pick, resized,
    revoke_frame_child, running, set_artifact_states, set_focus, set_presence_visible, settle,
    show_block, show_panel, start_frames, take_artifact_outcome, take_artifact_watch,
    take_bar_actions, take_block_pick, take_child_commits, take_child_menu_picks, take_created,
    take_focus_report, take_leaving, take_pick_answers, take_view_changes,
};
pub(crate) use runtime::{
    RegionPlacement, RegionSlot, RegionView, back_region, forward_region, frames, mount_region,
    place_region, region_damage, region_drawing, region_placed, region_view, take_changed,
    take_region_actions, unmount_region, unplace_region,
};
#[cfg(all(
    feature = "web-view",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]
pub(crate) use web_view::install as install_web_view;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn cache_in(directory: std::path::PathBuf) {
    wasm::cache_in(directory);
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
    },
    Panel(block_plugin_api::HostPanel),
    Creation {
        editor: Uuid,
        template: String,
    },
}

impl HostContent {
    pub(crate) fn block_id(&self) -> Option<Uuid> {
        match self {
            Self::Block { block_id, .. } => Some(*block_id),
            Self::Panel(_) | Self::Creation { .. } => None,
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
        matches!(self.mode, ChildMode::Active | ChildMode::Live)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HostChildStatus {
    pub(crate) child: ChildId,
    pub(crate) available: bool,
    pub(crate) intrinsic: Option<Vec2>,
    pub(crate) aspect_ratio: Option<f32>,
    pub(crate) hovered: bool,
    pub(crate) active: bool,
    pub(crate) interaction: InteractionMode,
    pub(crate) capabilities: EditorCapabilities,
    pub(crate) resize: ResizeMode,
    pub(crate) error: Option<String>,
    pub(crate) menu: Vec<block_plugin_api::MenuEntry>,
    pub(crate) creation: Option<block_plugin_api::CreationProgress>,
}

#[derive(Clone, Debug)]
pub(crate) struct ChildCommit {
    pub(crate) child: ChildId,
    pub(crate) parent: be_graph::BlockParent,
    pub(crate) name: Option<String>,
}

pub(crate) struct BlockPickRequest {
    pub(crate) request_id: u64,
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
    pub(crate) generation: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
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
    pub(crate) used: Option<Vec2>,
    pub(crate) placement: Option<[u32; 4]>,
    pub(crate) drawn: bool,
    pub(crate) children: usize,
    pub(crate) child_generation: u64,
}

pub(crate) fn preview_size(size: Vec2, scale_factor: f32) -> Vec2 {
    const STEP: f32 = 64.0;
    const MAXIMUM: f32 = 2048.0;
    let scale = scale_factor.max(f32::EPSILON);
    let pixels = size * scale;
    vec2(
        ((pixels.x / STEP).ceil() * STEP).clamp(STEP, MAXIMUM),
        ((pixels.y / STEP).ceil() * STEP).clamp(STEP, MAXIMUM),
    ) / scale
}

pub(crate) struct CreationSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<block_plugin_api::Catalog>,
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
    Editor(EditorBlock),
    Creation(Uuid, &'static str),
    Artifact(Uuid, EditorBlock),
}

impl InstanceRole {
    pub(crate) fn block(self) -> Option<EditorBlock> {
        match self {
            Self::Editor(block) | Self::Artifact(_, block) => Some(block),
            Self::Creation(..) => None,
        }
    }
}

pub(crate) struct ArtifactSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<block_plugin_api::Catalog>,
    pub(crate) client_id: Uuid,
    pub(crate) instance: EditorInstanceId,
    pub(crate) source_type: Uuid,
    pub(crate) block: EditorBlock,
    pub(crate) data: &'a [u8],
    pub(crate) resync: bool,
}

pub(crate) enum ArtifactState {
    Starting,
    Described { source: Uuid, summary: String },
    Failed(String),
}
