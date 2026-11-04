use beui::Rect;
use beui_runner::renderer::Capture;
use beui_types::Image;

/// One display's picture and where it sits on the desktop, in points.
pub struct Shown {
    pub rect: Rect,
    pub scale: f32,
    pub picture: Capture,
}

/// The whole desktop as one picture, at the largest display scale: each display's
/// picture is placed at its position, a display at a smaller scale is enlarged to
/// match, and whatever no display covers is transparent.
pub fn compose(shown: &[Shown]) -> Option<Image> {
    let bounds = shown
        .iter()
        .map(|shown| shown.rect)
        .reduce(|bounds, rect| bounds.union(rect))?;
    let scale = shown.iter().map(|shown| shown.scale).fold(0.0, f32::max);
    if scale <= 0.0 {
        return None;
    }
    let pixels = |length: f32| (length * scale).round().max(0.0) as u32;
    let (width, height) = (pixels(bounds.width()), pixels(bounds.height()));
    if width == 0 || height == 0 {
        return None;
    }
    let mut rgba = vec![0; width as usize * height as usize * 4];
    for shown in shown {
        let picture = &shown.picture;
        if picture.width == 0 || picture.height == 0 {
            continue;
        }
        let left = pixels(shown.rect.min.x - bounds.min.x);
        let top = pixels(shown.rect.min.y - bounds.min.y);
        let right = pixels(shown.rect.max.x - bounds.min.x).min(width);
        let bottom = pixels(shown.rect.max.y - bounds.min.y).min(height);
        let (across, down) = (right.saturating_sub(left), bottom.saturating_sub(top));
        for y in 0..down {
            let from_y = (u64::from(y) * u64::from(picture.height) / u64::from(down)) as usize;
            let row = (top + y) as usize * width as usize;
            for x in 0..across {
                let from_x = (u64::from(x) * u64::from(picture.width) / u64::from(across)) as usize;
                let from = (from_y * picture.width as usize + from_x) * 4;
                let to = (row + (left + x) as usize) * 4;
                rgba[to..to + 4].copy_from_slice(&picture.rgba[from..from + 4]);
            }
        }
    }
    Some(Image::from_rgba(width, height, rgba))
}

#[cfg(test)]
mod tests;
