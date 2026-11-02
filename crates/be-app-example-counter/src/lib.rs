pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    counter => app::CounterApp,
});

#[cfg(test)]
mod tests;
