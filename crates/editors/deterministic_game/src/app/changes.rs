use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Wake, Waker};

use block::Block;
use block_client::BlockHandle;

pub(super) struct BlockChanges<B: Block> {
    future: Pin<Box<dyn Future<Output = ()>>>,
    changed: Rc<Cell<bool>>,
    wake: Arc<BlockWake>,
    marker: std::marker::PhantomData<B>,
}

impl<B: Block> BlockChanges<B> {
    pub(super) fn new(block: BlockHandle<B>, host: block_editor_plugin::Waker) -> Self {
        let changed = Rc::new(Cell::new(false));
        let observed = changed.clone();
        let future = Box::pin(async move {
            block
                .wait_until(move |_| {
                    observed.set(true);
                    false
                })
                .await;
        });
        Self {
            future,
            changed,
            wake: Arc::new(BlockWake {
                pending: AtomicBool::new(true),
                host,
            }),
            marker: std::marker::PhantomData,
        }
    }

    pub(super) fn take(&mut self) -> bool {
        if self.wake.pending.swap(false, Ordering::AcqRel) {
            let waker = Waker::from(self.wake.clone());
            let _ = self.future.as_mut().poll(&mut Context::from_waker(&waker));
        }
        self.changed.replace(false)
    }
}

struct BlockWake {
    pending: AtomicBool,
    host: block_editor_plugin::Waker,
}

impl Wake for BlockWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.pending.store(true, Ordering::Release);
        self.host.wake();
    }
}
