use std::{future::Future, sync::mpsc::Receiver, thread};

pub fn spawn_request<T>(future: impl Future<Output = T> + Send + 'static) -> Receiver<T>
where
    T: Send + 'static,
{
    let (sender, receiver) = crate::host::waking_channel();
    thread::Builder::new()
        .name("block-app-request".into())
        .spawn(move || {
            let result = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime.block_on(future),
                Err(error) => panic!("failed to start a request runtime: {error}"),
            };
            let _ = sender.send(result);
        })
        .unwrap_or_else(|error| panic!("failed to start request: {error}"));
    receiver
}
