pub mod app;

block_editor_beui::beui_plugin!(app::CounterApp, "../manifest.json");

#[cfg(test)]
mod tests;
