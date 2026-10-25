#[cfg(test)]
mod tests;

be_ops_bindgen::generate!({
    path: "wit",
    world: "gpu",
    local: ["be3:gpu/device@0.1.0"],
});
