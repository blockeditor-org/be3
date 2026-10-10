#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub(crate) use linux::{
    Programs, WindowSurface, after, before, close, create, exiting, focus, fullscreen, launch,
    listed, lock_due, replace_gpu, revision, running, set_blank_after, set_keyboard,
    set_lock_after, set_locked, start,
};
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::{
    Programs, WindowSurface, after, before, close, create, exiting, focus, fullscreen, launch,
    listed, replace_gpu, revision, running, start,
};
