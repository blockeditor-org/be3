#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub(crate) use linux::{
    FullscreenSurface, WindowSurface, after, before, close, create, exiting, launch, listed,
    replace_gpu, revision, running, set_keyboard, start,
};
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::{
    FullscreenSurface, WindowSurface, after, before, close, create, exiting, launch, listed,
    replace_gpu, revision, running, start,
};
