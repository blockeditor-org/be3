pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    counter => root::CounterRoot,
});

#[cfg(test)]
mod tests;
