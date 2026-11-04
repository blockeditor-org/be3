use std::future::Future;

pub trait Request<T>: Future<Output = T> + 'static {}

impl<T, F: Future<Output = T> + 'static> Request<T> for F {}

pub fn spawn_request(request: impl Request<()>) {
    wasm_bindgen_futures::spawn_local(request);
}
