use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use block_guest_echo::Echo;

use super::*;

fn echo() -> (Worker<InProcess<Echo>>, Arc<AtomicUsize>) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let woken = Arc::clone(&wakes);
    let waiting: Arc<std::sync::atomic::AtomicBool> = Arc::default();
    let channel = InProcess::start(
        Arc::new(move || {
            woken.fetch_add(1, Ordering::SeqCst);
        }),
        Arc::clone(&waiting),
    );
    (Worker::new(channel, waiting), wakes)
}

fn polled(worker: &mut Worker<InProcess<Echo>>, steps: usize) -> Result<Vec<Vec<u8>>, String> {
    let mut received = Vec::new();
    for _ in 0..steps {
        if let Some(failure) = worker.poll() {
            return Err(failure);
        }
        received.extend(worker.take_received());
    }
    Ok(received)
}

mod an_in_process_guest_echoes_and_wakes;
mod an_in_process_guest_that_panics_reports_its_panic_message;
