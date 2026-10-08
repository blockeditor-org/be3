use block_editor_beui::be_block::display_settings::DisplayMode;
use block_editor_beui::{HostDisplay, HostDisplayMode};

pub(crate) type Size = (u32, u32);

pub(crate) fn saved(mode: HostDisplayMode) -> DisplayMode {
    DisplayMode {
        width: mode.width,
        height: mode.height,
        refresh_millihertz: mode.refresh_millihertz,
    }
}

fn offered(mode: DisplayMode) -> HostDisplayMode {
    HostDisplayMode {
        width: mode.width,
        height: mode.height,
        refresh_millihertz: mode.refresh_millihertz,
    }
}

pub(crate) fn size_of(mode: HostDisplayMode) -> Size {
    (mode.width, mode.height)
}

pub(crate) fn effective(display: &HostDisplay, saved: Option<DisplayMode>) -> HostDisplayMode {
    saved
        .map(offered)
        .filter(|mode| display.modes.contains(mode))
        .unwrap_or(display.default)
}

pub(crate) fn sizes(modes: &[HostDisplayMode]) -> Vec<Size> {
    let mut sizes: Vec<Size> = Vec::new();
    for mode in modes {
        if !sizes.contains(&size_of(*mode)) {
            sizes.push(size_of(*mode));
        }
    }
    sizes
}

pub(crate) fn rates(modes: &[HostDisplayMode], size: Size) -> Vec<HostDisplayMode> {
    modes
        .iter()
        .copied()
        .filter(|mode| size_of(*mode) == size)
        .collect()
}

pub(crate) fn size_label((width, height): Size) -> String {
    format!("{width} × {height}")
}

pub(crate) fn rate_label(refresh_millihertz: u32) -> String {
    let hertz = format!("{:.2}", f64::from(refresh_millihertz) / 1000.0);
    let hertz = hertz.trim_end_matches('0').trim_end_matches('.');
    format!("{hertz} Hz")
}

pub(crate) fn mode_label(mode: HostDisplayMode) -> String {
    format!(
        "{} at {}",
        size_label(size_of(mode)),
        rate_label(mode.refresh_millihertz)
    )
}
