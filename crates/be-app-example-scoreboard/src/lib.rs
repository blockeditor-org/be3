pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    scoreboard => root::ScoreboardRoot,
});

#[cfg(test)]
mod tests;
