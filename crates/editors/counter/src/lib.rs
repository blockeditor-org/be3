pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    counter => app::CounterApp,
});

#[cfg(test)]
mod tests;
