#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod shown;
#[cfg(any(windows, test))]
mod texture_draw;
#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(windows)]
mod win32;
#[cfg(any(windows, test))]
mod win32_map;
#[cfg(any(windows, test))]
mod win32_shape;

#[cfg(target_os = "linux")]
pub(crate) use linux::{
    Opener, Programs, WindowActions, WindowSurface, after, before, close, create, environment,
    exec, exiting, focus, fullscreen, hide, launch, listed, opening_windows, replace_gpu, revision,
    running, set_blank_after, set_keyboard, set_lock_after, set_locked, set_owned,
    set_primary_selection, show, start, take_lock_due,
};
#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) use unsupported::{
    Opener, Programs, WindowActions, WindowSurface, after, before, close, create, environment,
    exec, exiting, focus, fullscreen, hide, launch, listed, opening_windows, replace_gpu, revision,
    running, set_owned, show, start,
};
#[cfg(windows)]
pub(crate) use win32::{
    Opener, Programs, WindowActions, WindowSurface, after, before, close, create, environment,
    exec, exiting, focus, frame, fullscreen, hide, launch, listed, opening_windows, replace_gpu,
    revision, running, set_owned, show, start,
};
