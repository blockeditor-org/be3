mod compositor;
mod decoration;
mod render;
mod server;
mod state;
mod view;
mod windows;

pub use compositor::{Compositor, CursorImage};
pub use server::Server;
pub use state::{KeyboardConfig, WindowId};
pub use view::{FullscreenWindow, WindowView};
pub use windows::{Fullscreen, WindowInfo, Windows};

#[cfg(test)]
mod test_client;
