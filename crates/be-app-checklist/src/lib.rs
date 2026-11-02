pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    checklist => root::ChecklistRoot,
});

#[cfg(test)]
mod tests;
