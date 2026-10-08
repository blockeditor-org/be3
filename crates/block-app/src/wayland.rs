#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub(crate) use linux::{
    Programs, WindowSurface, after, before, close, create, fullscreen, exiting, launch, listed, replace_gpu,
    revision, running, set_keyboard, start,
};
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::{
    Programs, WindowSurface, after, before, close, create, fullscreen, exiting, launch, listed, replace_gpu,
    revision, running, start,
};
