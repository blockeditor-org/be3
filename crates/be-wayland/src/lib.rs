mod activation;
mod compositor;
mod decoration;
mod dialog;
mod focus;
mod idle;
pub mod programs;
mod render;
mod server;
mod state;
mod view;
mod windows;
mod xwayland;

pub use compositor::{Compositor, CursorImage};
pub use server::Server;
pub use state::{KeyboardConfig, WindowId};
pub use view::WindowView;
pub use windows::{Launch, WindowInfo, Windows};

#[cfg(test)]
mod test_client;
