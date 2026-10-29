use beui::{Event, Pos2, Rect, Replay, Replayed, Vec2};
use block_plugin_api::{
    DroppedFile, Input, InputBatch, Pasted, ScreenId, ScreenPlacement, Size, TimedInput,
    ViewportMetrics,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockDrag {
    pub(crate) block_id: Uuid,
    pub(crate) block_type: Uuid,
}

pub(super) struct BlockDragEvent {
    pub(super) position: Pos2,
    pub(super) drag: BlockDrag,
    pub(super) dropped: bool,
}

pub(super) struct FileDropEvent {
    pub(super) position: Pos2,
    pub(super) files: Vec<DroppedFile>,
    pub(super) dropped: bool,
}

pub(super) fn file_drop(input: &beui::ForwardedInput, dropping: bool) -> Option<FileDropEvent> {
    let mut hovering = dropping;
    let mut dropped = false;
    let mut files = Vec::new();
    for event in input.inputs() {
        match event {
            Event::FileHovered => hovering = true,
            Event::FileHoverCancelled => hovering = false,
            Event::FileDropped(file) => {
                dropped = true;
                files.extend(read_dropped(file.clone()));
            }
            _ => {}
        }
    }
    if !hovering && !dropped {
        return None;
    }
    let pointer = input.pointer.filter(|_| input.hovered)?;
    Some(FileDropEvent {
        position: Pos2::ZERO + (pointer - input.rect.min),
        files,
        dropped,
    })
}

fn read_dropped(file: beui::DroppedFile) -> Option<DroppedFile> {
    let name = Some(file.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "File".to_owned());
    let data = match file.bytes {
        Some(bytes) => bytes.to_vec(),
        None => read_path(file.path.as_ref()?)?,
    };
    Some(DroppedFile { name, data })
}

fn read_path(path: &std::path::Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

#[derive(Default)]
pub(super) struct InputForwarder {
    replay: Replay,
}

impl InputForwarder {
    pub(super) fn focused(&self) -> bool {
        self.replay.focused()
    }

    pub(super) fn forward(
        &mut self,
        input: &beui::ForwardedInput,
        screen: ScreenId,
        zoom: f32,
        every_sample: bool,
        aim: &dyn Fn(Option<beui::Over>, Pos2) -> beui::Aim,
        screen_of: &dyn Fn(beui::NodeId) -> Option<ScreenId>,
    ) -> Vec<super::calls::PluginCall> {
        let mut batch = InputBatch::new(screen, every_sample);
        for Replayed { input, time, over } in self.replay.replay(input, zoom, aim) {
            match input {
                Input::Paste(Pasted::Text(text)) => {
                    for piece in block_plugin_api::paste_events(&text) {
                        batch.push(piece);
                    }
                }
                input => batch.push(TimedInput {
                    input,
                    time_micros: time.map(block_app_base::host::micros_at),
                    over: over.and_then(screen_of),
                }),
            }
        }
        match batch.events.is_empty() {
            true => Vec::new(),
            false => vec![super::calls::PluginCall::Input(batch)],
        }
    }

    pub(super) fn intercepted(
        &mut self,
        press: Option<beui::KeyPress>,
        modifiers: beui::Modifiers,
    ) -> Vec<TimedInput> {
        self.replay
            .intercepted(press, modifiers)
            .into_iter()
            .map(TimedInput::from)
            .collect()
    }
}

pub(super) fn finer(placement: &ScreenPlacement) -> f32 {
    placement.raster() / placement.scale_factor()
}

pub(super) fn requested_at(metrics: &ViewportMetrics, finer: f32) -> (u32, u32) {
    let scale = finer * metrics.scale_factor / metrics.raster;
    match scale.is_finite() && (scale - 1.0).abs() > 1e-3 {
        true => (
            (metrics.pixel_width as f32 * scale).round() as u32,
            (metrics.pixel_height as f32 * scale).round() as u32,
        ),
        false => (metrics.pixel_width, metrics.pixel_height),
    }
}

pub(super) fn viewport_metrics(
    size: Vec2,
    visible: Rect,
    scale_factor: f32,
    raster: f32,
) -> ViewportMetrics {
    let logical = Size {
        width: size.x.max(0.0),
        height: size.y.max(0.0),
    };
    let visible = visible.intersect(Rect::from_min_size(Pos2::ZERO, size));
    ViewportMetrics {
        logical,
        visible: Pos2::new(visible.min.x.max(0.0), visible.min.y.max(0.0)),
        pixel_width: (visible.width().max(0.0) * raster).round() as u32,
        pixel_height: (visible.height().max(0.0) * raster).round() as u32,
        scale_factor,
        raster,
    }
}
