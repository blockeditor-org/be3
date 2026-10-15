use beui::{Color32, FrameOutput, Vec2};
use paint_snapshot::Snapshot;

pub(crate) fn capture(
    output: &FrameOutput,
    size: Vec2,
    pixels_per_point: f32,
    background: Color32,
) -> Result<Snapshot, String> {
    Snapshot::of_painting(beui::painting::painting(
        output,
        size,
        pixels_per_point,
        background,
        beui::painting::Punches::Shown,
    )?)
}
