pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    calendar => app::CalendarApp,
});

#[cfg(test)]
mod tests;
