pub mod app;

block_editor_plugin::beui_plugin!(app::VersionControlDataApp, "../manifest.json");

#[cfg(test)]
mod tests;
