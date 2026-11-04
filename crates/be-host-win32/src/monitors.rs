use crate::{Handle, Rect};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Monitor {
    pub handle: Handle,
    pub device: String,
    pub name: String,
    pub rect: Rect,
    pub work_area: Rect,
    pub scale: f32,
    pub primary: bool,
}

pub fn name(friendly: Option<&str>, description: Option<&str>, index: usize) -> String {
    [friendly, description]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|name| !name.is_empty())
        .map_or_else(|| format!("Display {}", index + 1), str::to_owned)
}

pub fn sort(monitors: &mut [Monitor]) {
    monitors.sort_by(|a, b| {
        b.primary
            .cmp(&a.primary)
            .then(a.rect.left.cmp(&b.rect.left))
            .then(a.rect.top.cmp(&b.rect.top))
            .then_with(|| a.device.cmp(&b.device))
    });
}
