mod activation;
mod compositor;
mod content_type;
mod decoration;
mod dialog;
mod focus;
mod foreign;
mod idle;
mod presentation;
pub mod programs;
mod render;
mod selection;
mod server;
mod shortcuts_inhibit;
mod single_pixel;
mod state;
mod tearing;
mod toplevel_icon;
mod view;
mod windows;
mod xwayland;

pub use compositor::{Compositor, CursorImage};
pub use presentation::Presented;
pub use server::Server;
pub use state::{KeyboardConfig, WindowId};
pub use view::{WindowView, register_restore_shortcuts};
pub use windows::{Launch, WindowInfo, Windows};

#[cfg(test)]
mod test_client;
