#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod shown;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub(crate) use linux::{
    Opener, Programs, WindowActions, WindowSurface, after, before, close, create, environment,
    exec, exiting, focus, fullscreen, hide, launch, listed, opening_windows, replace_gpu, revision,
    running, show, set_blank_after, set_keyboard, set_lock_after, set_locked, set_owned,
    set_primary_selection, start, take_lock_due,
};
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::{
    Opener, Programs, WindowActions, WindowSurface, after, before, close, create, environment,
    exec, exiting, focus, fullscreen, hide, launch, listed, opening_windows, replace_gpu, revision,
    running, show, set_owned, start,
};
