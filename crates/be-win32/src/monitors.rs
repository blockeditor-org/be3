use crate::{Handle, Rect};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Monitor {
    pub handle: Handle,
    /// `MONITORINFOEXW::szDevice`, such as `\\.\DISPLAY1`: what stays the same
    /// while handles change with the display configuration.
    pub device: String,
    pub name: String,
    /// In the virtual desktop's physical pixels.
    pub rect: Rect,
    pub work_area: Rect,
    /// Physical pixels per point, `GetDpiForMonitor`'s effective DPI over 96.
    pub scale: f32,
    pub primary: bool,
}

/// The name to show for a monitor: its friendly name from the display
/// configuration, else the adapter's description of it, else its number.
pub fn name(friendly: Option<&str>, description: Option<&str>, index: usize) -> String {
    [friendly, description]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|name| !name.is_empty())
        .map_or_else(|| format!("Display {}", index + 1), str::to_owned)
}

/// The primary monitor first, then left to right, then top to bottom.
pub fn sort(monitors: &mut [Monitor]) {
    monitors.sort_by(|a, b| {
        b.primary
            .cmp(&a.primary)
            .then(a.rect.left.cmp(&b.rect.left))
            .then(a.rect.top.cmp(&b.rect.top))
            .then_with(|| a.device.cmp(&b.device))
    });
}
