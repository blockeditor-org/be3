use std::{future::Future, sync::mpsc::Receiver};

pub(crate) fn spawn_request<T>(future: impl Future<Output = T> + 'static) -> Receiver<T>
where
    T: 'static,
{
    let (sender, receiver) = crate::host::waking_channel();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = sender.send(future.await);
    });
    receiver
}
