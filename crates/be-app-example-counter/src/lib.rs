pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    counter => root::CounterRoot,
    counter_host => root::CounterHost,
});

#[cfg(test)]
mod tests;
