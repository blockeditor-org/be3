//! A local interface (guides/type_modules.md, "Local interfaces") with what wasi:webgpu asks of
//! the generator: async funcs, futures and streams settled through the poll, options of lists
//! and strings passed flat, `result`s with nothing on success, a call too wide to pass flat,
//! and a resource another package's interface uses.

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "test",
    local: ["be3:shapes/shapes@0.1.0", "be3:local-test/frames@0.1.0"],
});

#[cfg(test)]
mod tests;
