use beui::{Color32, Vec2, WindowOutput};
use paint_snapshot::Snapshot;

pub(crate) fn capture(
    output: &WindowOutput,
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

pub(crate) fn capture_window(
    output: &WindowOutput,
    pixels_per_point: f32,
    background: Color32,
) -> Result<Snapshot, String> {
    Snapshot::of_painting(beui::painting::painting(
        output,
        output.size,
        pixels_per_point,
        background,
        beui::painting::Punches::Shown,
    )?)
}
