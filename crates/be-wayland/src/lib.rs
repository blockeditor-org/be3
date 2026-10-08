mod compositor;
mod decoration;
pub mod programs;
mod render;
mod server;
mod state;
mod view;
mod windows;

pub use compositor::{Compositor, CursorImage};
pub use server::Server;
pub use state::{KeyboardConfig, WindowId};
pub use view::WindowView;
pub use windows::{Launch, WindowInfo, Windows};

#[cfg(test)]
mod test_client;
