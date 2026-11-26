//! The remaps as a pure guest (guides/app_libraries.md, Pure guests), which be-host runs in its
//! sandbox: each frame it is sent is a request, answered with one frame.

use be_lib_remap::Module;
use be_wasm_guest_abi::{Flow, Guest, Waker};

pub struct Remaps(Module);

impl Guest for Remaps {
    fn start(_waker: Waker, _outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self(Module::default()))
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        outbound.extend(inbound.iter().map(|request| self.0.handle(request)));
        Ok(Flow::Running)
    }
}

be_wasm_guest_abi::guest!(Remaps);
