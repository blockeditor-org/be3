//! `host_panic`, through which be-wasm-guest-abi's panic hook hands the host the message a guest
//! panicked with: kept in the State for the call that ran the guest to report, then thrown, which
//! unwinds the guest.

use be_wasm_guest_abi as abi;

use crate::engine::Defined;
use crate::gpu::throw;
use crate::state::state_of;

fn host_panic(
    scope: &mut v8::PinScope,
    arguments: v8::FunctionCallbackArguments,
    _: v8::ReturnValue<v8::Value>,
) {
    // the function's data is its State (state_of).
    // unsafe(memory)
    let state = unsafe { state_of(arguments.data()) };
    let pointer = arguments.get(0).uint32_value(scope).unwrap_or(0);
    let length = arguments.get(1).uint32_value(scope).unwrap_or(0);
    let message = match state.view_in(scope).read(pointer, length) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => format!("the app panicked, and its message was lost: {error}"),
    };
    throw(scope, &message);
    state.panicked = Some(message);
}

pub(crate) fn define<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    defined: &mut Defined<'s>,
    data: v8::Local<'s, v8::Value>,
) -> Result<(), String> {
    crate::engine::function(scope, defined, abi::HOST_MODULE, abi::HOST_PANIC, data, host_panic)
}
