#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub(crate) use linux::{
    Opener, Programs, WindowActions, WindowSurface, after, before, close, create, environment,
    exec, exiting, focus, fullscreen, launch, listed, opening_windows, replace_gpu, revision,
    running, set_blank_after, set_keyboard, set_lock_after, set_locked, set_owned,
    set_primary_selection, start, take_lock_due,
};
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::{
    Opener, Programs, WindowActions, WindowSurface, after, before, close, create, environment,
    exec, exiting, focus, fullscreen, launch, listed, opening_windows, replace_gpu, revision,
    running, set_owned, start,
};
