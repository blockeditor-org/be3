mod activation;
mod compositor;
mod content_type;
mod decoration;
mod dialog;
mod focus;
mod foreign;
mod gtk_settings;
mod heads;
mod held_view;
mod idle;
mod presentation;
pub mod programs;
mod render;
mod runtime_dir;
mod selection;
mod server;
mod shortcuts_inhibit;
mod single_pixel;
mod state;
mod tablet;
mod tearing;
mod text_input;
mod toplevel_icon;
mod touch;
mod view;
mod windows;
mod xwayland;

pub use compositor::{Compositor, CursorImage, Monitor};
pub use presentation::Presented;
pub use server::Server;
pub use state::{KeyboardConfig, WindowId};
pub use view::{WindowView, register_restore_shortcuts};
pub use windows::{Launch, Popups, WindowInfo, Windows};

#[cfg(test)]
mod test_client;
