const POINTS_PER_INCH: f32 = 110.0;
const STEP: f32 = 0.25;
const MIN_SCALE: f32 = 1.0;
const MAX_SCALE: f32 = 3.0;
const MIN_MILLIMETRES: u32 = 40;
const MILLIMETRES_PER_INCH: f32 = 25.4;

pub fn natural_scale(pixels: (u32, u32), millimetres: Option<(u32, u32)>) -> f32 {
    let Some((width_mm, height_mm)) = millimetres else {
        return MIN_SCALE;
    };
    if width_mm < MIN_MILLIMETRES || height_mm < MIN_MILLIMETRES {
        return MIN_SCALE;
    }
    let diagonal = |a: f32, b: f32| a.hypot(b);
    let inches = diagonal(width_mm as f32, height_mm as f32) / MILLIMETRES_PER_INCH;
    let dots_per_inch = diagonal(pixels.0 as f32, pixels.1 as f32) / inches;
    let scale = (dots_per_inch / POINTS_PER_INCH / STEP).round() * STEP;
    scale.clamp(MIN_SCALE, MAX_SCALE)
}

#[cfg(test)]
mod tests;
