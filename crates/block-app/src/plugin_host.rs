use std::sync::Arc;

use beui::{Pos2, Rect, Vec2, vec2};
use block_client::BlockClient;
use block_plugin_api::{
    BlockTypeDescriptor, ChildId, ChildLayer, ChildMode, EditorCapabilities, EditorInstanceId,
    EditorRegion, FrameSpec, InteractionMode, PluginManifest, ResizeMode, ScreenId,
};
use uuid::Uuid;

mod audio;
mod backend;
mod clipboard;
mod input;
mod instances;
mod pieces;
mod presenter;
mod runtime;
#[cfg(not(target_arch = "wasm32"))]
mod wasm;
#[cfg(target_arch = "wasm32")]
mod web;
mod web_view;

pub(crate) use instances::EditorView;
pub(crate) use presenter::{Blit, PluginDrawing};
pub(crate) use runtime::{
    artifact, artifact_draft, aspect_ratio, block_picked, close, commit_creation, cover_frame,
    creation, creation_ready, editor_ui, flush, frame_child, frame_rects, hold, install,
    intrinsic_size, kill, poll, present, presenting, preview, regenerate_artifact, region_size,
    replace_child, report_child_views, report_children, resized, revoke_frame_child, running,
    set_artifact_states, set_focus, set_presence_visible, show_block, take_artifact_outcome,
    take_artifact_watch, take_block_pick, take_created, take_focus_report, take_leaving,
    take_view_changes,
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

pub(crate) struct HostChild {
    pub(crate) child: ChildId,
    pub(crate) frame_owner: bool,
    pub(crate) own_frame: bool,
    pub(crate) top_bar: bool,
    pub(crate) block_id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) rect: Rect,
    pub(crate) clip: Rect,
    pub(crate) layer: ChildLayer,
    pub(crate) mode: ChildMode,
    pub(crate) intrinsic: Option<Vec2>,
    pub(crate) rotation: f32,
    pub(crate) opacity: f32,
}

impl HostChild {
    pub(crate) fn is_below(&self) -> bool {
        matches!(self.layer, ChildLayer::Below)
    }

    pub(crate) fn is_active(&self) -> bool {
        matches!(self.mode, ChildMode::Active | ChildMode::Live)
    }

    pub(crate) fn is_preview(&self) -> bool {
        matches!(self.mode, ChildMode::Preview)
    }
}

pub(crate) struct PreviewPresentation {
    pub(crate) drawn: bool,
    pub(crate) size: Vec2,
    pub(crate) children: Vec<HostChild>,
}

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
}

pub(crate) struct BlockPickRequest {
    pub(crate) request_id: u64,
    pub(crate) block_types: Vec<Uuid>,
    pub(crate) excluded: Vec<Uuid>,
    pub(crate) templates: bool,
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

pub(crate) struct PreviewSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) client_id: Uuid,
    pub(crate) block_id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) instance: EditorInstanceId,
    pub(crate) corners: [Pos2; 4],
    pub(crate) opacity: f32,
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
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) client_id: Uuid,
    pub(crate) instance: EditorInstanceId,
}

pub(crate) enum CreationState {
    Starting,
    Ready,
    Failed(String),
}

pub(crate) struct EditorSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) client_id: Uuid,
    pub(crate) role: InstanceRole,
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: EditorRegion,
    pub(crate) frame: Option<FrameSpec>,
    pub(crate) size: Vec2,
    pub(crate) view: Option<EditorView>,
}

#[derive(Clone, Copy)]
pub(crate) struct EditorBlock {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
}

#[derive(Clone, Copy)]
pub(crate) enum InstanceRole {
    Editor(EditorBlock),
    Creation,
    Artifact(EditorBlock),
}

impl InstanceRole {
    pub(crate) fn block(self) -> Option<EditorBlock> {
        match self {
            Self::Editor(block) | Self::Artifact(block) => Some(block),
            Self::Creation => None,
        }
    }
}

pub(crate) struct ArtifactSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) client_id: Uuid,
    pub(crate) instance: EditorInstanceId,
    pub(crate) block: EditorBlock,
    pub(crate) data: &'a [u8],
    pub(crate) resync: bool,
}

pub(crate) enum ArtifactState {
    Starting,
    Described { source: Uuid, summary: String },
    Failed(String),
}
