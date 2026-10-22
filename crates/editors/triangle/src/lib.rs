pub mod app;

block_editor_plugin::plugin!("../manifest.json", {
    triangle => app::TrianglePlugin,
});

#[cfg(test)]
mod tests;
