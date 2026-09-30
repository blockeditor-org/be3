#[cfg(any(test, target_arch = "wasm32"))]
mod order;
#[cfg(any(test, target_arch = "wasm32"))]
mod style;

#[cfg(target_arch = "wasm32")]
mod scene;

#[cfg(target_arch = "wasm32")]
pub use scene::DomRenderer;

#[cfg(test)]
mod tests;
