pub mod root;

be_sdk::app!("../manifest.json", {
    triangle => root::TriangleApp,
});

#[cfg(test)]
mod tests;
