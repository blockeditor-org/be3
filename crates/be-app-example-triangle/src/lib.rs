pub mod root;

be_sdk::plugin!("../manifest.json", {
    triangle => root::TrianglePlugin,
});

#[cfg(test)]
mod tests;
