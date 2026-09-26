pub mod app;

block_editor_plugin::plugin!(app::TrianglePlugin, "../manifest.json");

#[cfg(test)]
mod tests;
