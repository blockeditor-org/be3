pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    counter => root::CounterRoot,
});

#[cfg(test)]
mod tests;
