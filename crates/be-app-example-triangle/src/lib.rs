pub mod app;

be_sdk::plugin!("../manifest.json", {
    triangle => app::TrianglePlugin,
});

#[cfg(test)]
mod tests;
