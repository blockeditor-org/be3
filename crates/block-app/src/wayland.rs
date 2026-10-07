#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub(crate) use linux::{
    WindowSurface, after, before, close, create, exiting, launch, listed, revision, running,
    set_keyboard, start,
};
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::{
    WindowSurface, after, before, close, create, exiting, launch, listed, revision, running, start,
};
