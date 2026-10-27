mod luid;
mod memory;
mod ring;
mod size;

pub use luid::{Luid, adapter_matching, luid};
pub use memory::memory_type;
pub use ring::{Pick, Ring};
pub use size::{Size, copied, pool_after};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Monitor(isize),
    Window(isize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Frame,
    Lost,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lost {
    Closed,
    Failed(String),
}

#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod gpu;
#[cfg(windows)]
mod handle;

#[cfg(windows)]
pub use capture::{Capture, Frame};
#[cfg(windows)]
pub use gpu::{DEVICE_EXTENSIONS, Device};

#[cfg(test)]
mod tests;
