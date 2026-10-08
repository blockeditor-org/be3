mod compositor;
mod decoration;
mod idle;
pub mod programs;
mod render;
mod server;
mod state;
mod view;
mod windows;

pub use compositor::{Compositor, CursorImage};
pub use server::Server;
pub use state::{KeyboardConfig, WindowId};
pub use view::{WindowView, toggle_fullscreen_action};
pub use windows::{Launch, WindowInfo, Windows};

#[cfg(test)]
mod test_client;
