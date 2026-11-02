pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    checklist => root::ChecklistRoot,
});

#[cfg(test)]
mod tests;
