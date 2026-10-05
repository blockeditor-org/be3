mod compositor;
mod render;
mod server;
mod state;
mod view;
mod windows;

pub use compositor::{Compositor, CursorImage};
pub use server::Server;
pub use state::WindowId;
pub use view::WindowView;
pub use windows::{WindowInfo, Windows};

#[cfg(test)]
mod test_client;
