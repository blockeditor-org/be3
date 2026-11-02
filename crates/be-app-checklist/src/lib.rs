pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    checklist => app::ChecklistApp,
});

#[cfg(test)]
mod tests;
