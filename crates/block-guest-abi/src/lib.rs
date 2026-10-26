pub mod direct;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

use std::sync::Arc;

pub const HOST_MODULE: &str = "be3_host";
pub const HOST_SEND: &str = "host_send";
pub const HOST_RECEIVE: &str = "host_receive";
pub const HOST_WAKE: &str = "host_wake";
pub const HOST_PANIC: &str = "host_panic";

pub const START: &str = "plugin_start";
pub const STEP: &str = "plugin_step";
pub const SHUTDOWN: &str = "plugin_shutdown";

pub const NO_MESSAGE: i64 = -1;

#[macro_export]
macro_rules! transport_functions {
    ($callback:path) => {
        $callback! {
            fn host_send(pointer: u32, length: u32);
            fn host_receive(pointer: u32, capacity: u32) -> i64;
        }
    };
}

#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(wake))
    }

    pub fn wake(&self) {
        (self.0)();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Running,
    Finished,
}

pub trait Guest: Sized + 'static {
    fn start(waker: Waker, outbound: &mut Vec<Vec<u8>>) -> Result<Self, String>;

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String>;
}

#[macro_export]
macro_rules! guest {
    ($guest:ty) => {
        $crate::guest!($guest, || {});
    };
    ($guest:ty, $before:expr) => {
        #[cfg(target_arch = "wasm32")]
        const _: () = {
            ::std::thread_local! {
                static GUEST: ::core::cell::RefCell<::core::option::Option<$guest>> =
                    const { ::core::cell::RefCell::new(::core::option::Option::None) };
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn plugin_start() {
                ($before)();
                $crate::wasm::start(&GUEST);
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn plugin_step() {
                $crate::wasm::step(&GUEST);
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn plugin_shutdown() {
                $crate::wasm::shutdown(&GUEST);
            }
        };
    };
}
