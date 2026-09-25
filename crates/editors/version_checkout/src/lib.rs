pub mod app;

block_editor_plugin::beui_plugin!(app::CheckoutApp, "../manifest.json");

#[cfg(test)]
mod tests;
