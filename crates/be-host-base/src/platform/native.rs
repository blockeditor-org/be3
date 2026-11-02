use std::{future::Future, thread};

pub trait Request<T>: Future<Output = T> + Send + 'static {}

impl<T, F: Future<Output = T> + Send + 'static> Request<T> for F {}

pub fn spawn_request(request: impl Request<()>) {
    thread::Builder::new()
        .name("block-app-request".into())
        .spawn(move || match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime.block_on(request),
            Err(error) => panic!("failed to start a request runtime: {error}"),
        })
        .unwrap_or_else(|error| panic!("failed to start request: {error}"));
}
