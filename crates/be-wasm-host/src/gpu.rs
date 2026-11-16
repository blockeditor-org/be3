//! The gpu and transport imports, one host function each, generated from the ABI's own lists
//! (`gpu_local_imports!`, `transport_functions!`) as the browser's are.
//!
//! Where the guest's memory is shared, each is a fast-API function that V8 calls straight from the
//! guest's code with the import's own arguments and its State in the callback's options: no
//! JavaScript and no handles, so it reaches the memory without V8 (`State::view`). A guest with a
//! memory of its own, which may move when it grows, gets the same functions made the ordinary way,
//! which read where the memory is through a scope on every call (`State::view_in`).
//!
//! A host call that must stop the guest (it reached the gpu and none could be opened) leaves why in
//! `State::fatal`: an ordinary function throws, and a fast one, which may not throw, terminates its
//! isolate, which stops the guest at its next call or loop.

use std::ffi::c_void;

use be_wasm_gpu_host::transport;
use v8::fast_api::{CFunction, CFunctionInfo, FastApiCallbackOptions, Int64Representation, Type};

use crate::engine::Defined;
use crate::state::{Guest, state_of};

/// A wasm value as a fast-API call and an ordinary call each carry it.
pub(crate) trait Wasm: Sized {
    const TYPE: Type;
    fn from_value(scope: &mut v8::PinScope<'_, '_>, value: v8::Local<v8::Value>) -> Self;
    fn give(self, scope: &mut v8::PinScope<'_, '_>, returned: &mut v8::ReturnValue<v8::Value>);
}

impl Wasm for () {
    const TYPE: Type = Type::Void;
    fn from_value(_: &mut v8::PinScope<'_, '_>, _: v8::Local<v8::Value>) -> Self {}
    fn give(self, _: &mut v8::PinScope<'_, '_>, _: &mut v8::ReturnValue<v8::Value>) {}
}

impl Wasm for i32 {
    const TYPE: Type = Type::Int32;
    fn from_value(scope: &mut v8::PinScope<'_, '_>, value: v8::Local<v8::Value>) -> Self {
        value.int32_value(scope).unwrap_or(0)
    }
    fn give(self, _: &mut v8::PinScope<'_, '_>, returned: &mut v8::ReturnValue<v8::Value>) {
        returned.set_int32(self);
    }
}

impl Wasm for u32 {
    const TYPE: Type = Type::Uint32;
    fn from_value(scope: &mut v8::PinScope<'_, '_>, value: v8::Local<v8::Value>) -> Self {
        value.uint32_value(scope).unwrap_or(0)
    }
    fn give(self, _: &mut v8::PinScope<'_, '_>, returned: &mut v8::ReturnValue<v8::Value>) {
        returned.set_int32(self as i32);
    }
}

impl Wasm for i64 {
    const TYPE: Type = Type::Int64;
    fn from_value(_: &mut v8::PinScope<'_, '_>, value: v8::Local<v8::Value>) -> Self {
        v8::Local::<v8::BigInt>::try_from(value).map_or(0, |value| value.i64_value().0)
    }
    fn give(self, scope: &mut v8::PinScope<'_, '_>, returned: &mut v8::ReturnValue<v8::Value>) {
        returned.set(v8::BigInt::new_from_i64(scope, self).into());
    }
}

impl Wasm for u64 {
    const TYPE: Type = Type::Uint64;
    fn from_value(_: &mut v8::PinScope<'_, '_>, value: v8::Local<v8::Value>) -> Self {
        v8::Local::<v8::BigInt>::try_from(value).map_or(0, |value| value.u64_value().0)
    }
    fn give(self, scope: &mut v8::PinScope<'_, '_>, returned: &mut v8::ReturnValue<v8::Value>) {
        returned.set(v8::BigInt::new_from_i64(scope, self as i64).into());
    }
}

impl Wasm for f32 {
    const TYPE: Type = Type::Float32;
    fn from_value(scope: &mut v8::PinScope<'_, '_>, value: v8::Local<v8::Value>) -> Self {
        value.number_value(scope).unwrap_or(0.0) as f32
    }
    fn give(self, _: &mut v8::PinScope<'_, '_>, returned: &mut v8::ReturnValue<v8::Value>) {
        returned.set_double(f64::from(self));
    }
}

impl Wasm for f64 {
    const TYPE: Type = Type::Float64;
    fn from_value(scope: &mut v8::PinScope<'_, '_>, value: v8::Local<v8::Value>) -> Self {
        value.number_value(scope).unwrap_or(0.0)
    }
    fn give(self, _: &mut v8::PinScope<'_, '_>, returned: &mut v8::ReturnValue<v8::Value>) {
        returned.set_double(self);
    }
}

macro_rules! returns {
    () => {
        ()
    };
    ($result:ty) => {
        $result
    };
}

/// Runs a host call's body so that a panic in it (in the gpu glue, say) stops the one guest rather
/// than unwinding into V8, which would abort the process.
pub(crate) fn contained<R>(call: impl FnOnce() -> R) -> Result<R, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)).map_err(|panic| {
        let message = panic
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        format!("a host call panicked: {message}")
    })
}

/// Throws an Error with `message` into the guest that called.
pub(crate) fn throw(scope: &mut v8::PinScope<'_, '_>, message: &str) {
    let message = v8::String::new(scope, message).unwrap_or_else(|| v8::String::empty(scope));
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
}

/// After a host call: stops the guest where the call left why.
fn stop_fast(state: &mut crate::state::State) {
    if state.fatal.is_some() {
        state.isolate.terminate_execution();
    }
}

fn stop_slow(scope: &mut v8::PinScope<'_, '_>, state: &mut crate::state::State) {
    if let Some(fatal) = &state.fatal {
        let fatal = fatal.clone();
        throw(scope, &fatal);
    }
}

/// Makes `defined[module][name]` a host function: fast-API when `fast`, else ordinary.
fn define<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    defined: &mut Defined<'s>,
    module: &str,
    name: &str,
    data: v8::Local<'s, v8::Value>,
    slow: impl v8::MapFnTo<v8::FunctionCallback>,
    fast: Option<&'static [CFunction]>,
) -> Result<(), String> {
    let template = v8::FunctionTemplate::builder(slow).data(data);
    let template = match fast {
        Some(fast) => template.build_fast(scope, fast),
        None => template.build(scope),
    };
    let function = template
        .get_function(scope)
        .ok_or_else(|| format!("{module}.{name} could not be made"))?;
    defined.define(module, name, function.into());
    Ok(())
}

macro_rules! bind_gpu {
    ($($glue:path, $module:literal, $name:literal, $export:literal, ($($argument:ident: $kind:ty),*) $(-> $result:ty)?;)*) => {
        fn define_gpu<'s>(
            scope: &mut v8::PinScope<'s, '_>,
            defined: &mut Defined<'s>,
            data: v8::Local<'s, v8::Value>,
            fast: bool,
        ) -> Result<(), String> {
            $({
                extern "C" fn fast_call(
                    _receiver: v8::Local<v8::Value>,
                    $($argument: $kind,)*
                    options: &FastApiCallbackOptions,
                ) -> returns!($($result)?) {
                    // V8 hands a fast call the options of the function it calls, whose
                    // data is its State (state_of).
                    let state = unsafe { state_of(options.data) };
                    let view = state.view();
                    let result = contained(|| $glue(&mut Guest::new(&mut *state, view), $($argument),*));
                    let result = result.unwrap_or_else(|panic| {
                        state.fatal.get_or_insert(panic);
                        Default::default()
                    });
                    stop_fast(state);
                    result
                }
                #[allow(unused_assignments)]
                fn slow_call(
                    scope: &mut v8::PinScope,
                    arguments: v8::FunctionCallbackArguments,
                    mut returned: v8::ReturnValue<v8::Value>,
                ) {
                    // the function's data is its State (state_of).
                    let state = unsafe { state_of(arguments.data()) };
                    #[allow(unused_mut, unused_variables, unused_assignments)]
                    let mut index = 0;
                    $(
                        let $argument = <$kind as Wasm>::from_value(scope, arguments.get(index));
                        index += 1;
                    )*
                    let view = state.view_in(scope);
                    let result = contained(|| $glue(&mut Guest::new(&mut *state, view), $($argument),*));
                    let result: returns!($($result)?) = result.unwrap_or_else(|panic| {
                        state.fatal.get_or_insert(panic);
                        Default::default()
                    });
                    stop_slow(scope, state);
                    result.give(scope, &mut returned);
                }
                const INFO: CFunctionInfo = CFunctionInfo::new(
                    <returns!($($result)?) as Wasm>::TYPE.as_info(),
                    &[
                        Type::V8Value.as_info(),
                        $(<$kind as Wasm>::TYPE.as_info(),)*
                        Type::CallbackOptions.as_info(),
                    ],
                    Int64Representation::BigInt,
                );
                const FAST: [CFunction; 1] = [CFunction::new(fast_call as *const c_void, &INFO)];
                define(scope, defined, $module, $name, data, slow_call, fast.then_some(&FAST))?;
            })*
            Ok(())
        }
    };
}

macro_rules! bind_transport {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)?;)+) => {
        pub(crate) fn define_transport<'s>(
            scope: &mut v8::PinScope<'s, '_>,
            defined: &mut Defined<'s>,
            data: v8::Local<'s, v8::Value>,
            fast: bool,
        ) -> Result<(), String> {
            $({
                extern "C" fn fast_call(
                    _receiver: v8::Local<v8::Value>,
                    $($argument: $kind,)*
                    options: &FastApiCallbackOptions,
                ) -> returns!($($result)?) {
                    // as in define_gpu.
                    let state = unsafe { state_of(options.data) };
                    let view = state.view();
                    let result = contained(|| transport::$name(&mut Guest::new(&mut *state, view), $($argument),*));
                    let result = result.unwrap_or_else(|panic| {
                        state.fatal.get_or_insert(panic);
                        Default::default()
                    });
                    stop_fast(state);
                    result
                }
                #[allow(unused_assignments)]
                fn slow_call(
                    scope: &mut v8::PinScope,
                    arguments: v8::FunctionCallbackArguments,
                    mut returned: v8::ReturnValue<v8::Value>,
                ) {
                    // as in define_gpu.
                    let state = unsafe { state_of(arguments.data()) };
                    #[allow(unused_mut, unused_variables, unused_assignments)]
                    let mut index = 0;
                    $(
                        let $argument = <$kind as Wasm>::from_value(scope, arguments.get(index));
                        index += 1;
                    )*
                    let view = state.view_in(scope);
                    let result = contained(|| transport::$name(&mut Guest::new(&mut *state, view), $($argument),*));
                    let result: returns!($($result)?) = result.unwrap_or_else(|panic| {
                        state.fatal.get_or_insert(panic);
                        Default::default()
                    });
                    stop_slow(scope, state);
                    result.give(scope, &mut returned);
                }
                const INFO: CFunctionInfo = CFunctionInfo::new(
                    <returns!($($result)?) as Wasm>::TYPE.as_info(),
                    &[
                        Type::V8Value.as_info(),
                        $(<$kind as Wasm>::TYPE.as_info(),)*
                        Type::CallbackOptions.as_info(),
                    ],
                    Int64Representation::BigInt,
                );
                const FAST: [CFunction; 1] = [CFunction::new(fast_call as *const c_void, &INFO)];
                define(scope, defined, be_wasm_guest_abi::HOST_MODULE, stringify!($name), data, slow_call, fast.then_some(&FAST))?;
            })+
            Ok(())
        }
    };
}

be_wasm_gpu_abi::gpu_local_imports!(bind_gpu);
be_wasm_guest_abi::transport_functions!(bind_transport);

/// Every gpu and transport import of an app: fast-API where its memory is shared.
pub(crate) fn define_app<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    defined: &mut Defined<'s>,
    data: v8::Local<'s, v8::Value>,
    fast: bool,
) -> Result<(), String> {
    define_gpu(scope, defined, data, fast)?;
    define_transport(scope, defined, data, fast)
}
