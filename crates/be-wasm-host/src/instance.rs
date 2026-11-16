//! Instantiating a module on an isolate: the host functions each kind of guest gets, how each of
//! a module's imports is answered, and `new WebAssembly.Instance` with them.

use be_wasm_gpu_abi::embed::device as gpu_abi;
use be_wasm_guest_abi as guest;

use crate::engine::{self, Defined, Import, Kind, Module};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// An app's main thread, whose gpu and transport calls are fast-API calls when its memory is
    /// shared.
    App { fast: bool },
    /// A pure guest: its transport, waking and panicking, and nothing else.
    Pure,
    /// A thread an app spawned: WASI, waking, panicking and spawning, and every other host call
    /// refused, since only the main thread may make them.
    Thread,
}

/// The host functions a guest of `role` is offered, made with `data` (its State).
pub(crate) fn host_functions<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    data: v8::Local<'s, v8::Value>,
    role: Role,
    modules: &[Module],
) -> Result<Defined<'s>, String> {
    let mut defined = Defined::default();
    crate::panic::define(scope, &mut defined, data)?;
    crate::wake::define(scope, &mut defined, data)?;
    let imports = || {
        modules
            .iter()
            .flat_map(|module| module.shape.imports.iter())
    };
    match role {
        Role::Pure => crate::gpu::define_transport(scope, &mut defined, data, false)?,
        Role::App { fast } => {
            crate::gpu::define_app(scope, &mut defined, data, fast)?;
            crate::wasi::define(
                scope,
                &mut defined,
                data,
                &imports().cloned().collect::<Vec<_>>(),
            )?;
            crate::threads::define(scope, &mut defined, data)?;
        }
        Role::Thread => {
            crate::wasi::define(
                scope,
                &mut defined,
                data,
                &imports().cloned().collect::<Vec<_>>(),
            )?;
            crate::threads::define(scope, &mut defined, data)?;
            for import in imports() {
                let host = import.module == guest::HOST_MODULE;
                if !(import.module == gpu_abi::MODULE || host)
                    || (host
                        && matches!(import.name.as_str(), guest::HOST_WAKE | guest::HOST_PANIC))
                {
                    continue;
                }
                let refusal = stub(
                    scope,
                    &format!(
                        "an app thread called {}::{}, which only its main thread may do",
                        import.module, import.name
                    ),
                )?;
                defined.define(&import.module, &import.name, refusal);
            }
        }
    }
    Ok(defined)
}

fn refuse(
    scope: &mut v8::PinScope,
    arguments: v8::FunctionCallbackArguments,
    _: v8::ReturnValue<v8::Value>,
) {
    let message = arguments.data().to_rust_string_lossy(scope);
    crate::gpu::throw(scope, &message);
}

/// A function that throws `message` when called.
pub(crate) fn stub<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    message: &str,
) -> Result<v8::Local<'s, v8::Value>, String> {
    let data = engine::key(scope, message);
    v8::Function::builder(refuse)
        .data(data)
        .build(scope)
        .map(Into::into)
        .ok_or_else(|| "a refusing host function could not be made".to_owned())
}

/// How a module's imports are answered: the memory, the host's functions, and for an app (not a
/// pure guest) a function that throws for anything else, as a host it does not answer.
pub(crate) struct Answers<'s> {
    pub(crate) defined: Defined<'s>,
    pub(crate) memory: Option<v8::Local<'s, v8::Object>>,
    pub(crate) pure: bool,
}

impl<'s> Answers<'s> {
    pub(crate) fn answer(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        import: &Import,
    ) -> Result<v8::Local<'s, v8::Value>, String> {
        let (module, name) = (&import.module, &import.name);
        if let Kind::Memory(_) = import.kind {
            return self.memory.map(Into::into).ok_or_else(|| {
                format!("the app imports the memory {module}.{name}, and has none")
            });
        }
        if let Some(defined) = self.defined.get(module, name) {
            return Ok(defined);
        }
        if self.pure || import.kind != Kind::Function {
            return Err(format!(
                "the app imports {module}.{name}, which its host does not answer"
            ));
        }
        stub(
            scope,
            &format!("the app called {module}.{name}, which its host does not answer"),
        )
    }
}

/// The module object of `module` on this isolate.
pub(crate) fn module_object<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    module: &Module,
) -> Result<v8::Local<'s, v8::WasmModuleObject>, String> {
    v8::WasmModuleObject::from_compiled_module(scope, &module.module)
        .ok_or_else(|| "a compiled module could not be used on a new isolate".to_owned())
}

/// `new WebAssembly.Instance(module, imports)` with `imports` in the module's order, as the
/// instance's exports.
pub(crate) fn instantiate<'a, 's>(
    scope: &mut v8::PinScope<'s, '_>,
    module: &Module,
    imports: impl IntoIterator<Item = (&'a Import, v8::Local<'s, v8::Value>)>,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let object = v8::Object::new(scope);
    for (import, value) in imports {
        let namespace = match engine::get(scope, object, &import.module)
            .and_then(|namespace| namespace.to_object(scope))
        {
            Some(namespace) => namespace,
            None => {
                let namespace = v8::Object::new(scope);
                engine::set(scope, object, &import.module, namespace.into());
                namespace
            }
        };
        engine::set(scope, namespace, &import.name, value);
    }
    let module = module_object(scope, module)?;
    let constructor = engine::web_assembly(scope, "Instance")?;
    let instance = constructor
        .new_instance(scope, &[module.into(), object.into()])
        .ok_or_else(|| "the app could not be instantiated".to_owned())?;
    engine::get(scope, instance, "exports")
        .and_then(|exports| exports.to_object(scope))
        .ok_or_else(|| "the app's instance has no exports".to_owned())
}

/// Instantiates `module` with every import answered by `answers`.
pub(crate) fn instantiate_answered<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    module: &Module,
    answers: &Answers<'s>,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let mut imports = Vec::with_capacity(module.shape.imports.len());
    for import in &module.shape.imports {
        imports.push((import, answers.answer(scope, import)?));
    }
    instantiate(scope, module, imports)
}

/// The function an instance exports as `name`, when it does.
pub(crate) fn function<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    exports: v8::Local<v8::Object>,
    name: &str,
) -> Option<v8::Local<'s, v8::Function>> {
    engine::get(scope, exports, name).and_then(|function| function.try_into().ok())
}

/// Calls `exports[name](...arguments)` when the instance exports it.
pub(crate) fn call_export<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    exports: v8::Local<v8::Object>,
    name: &str,
    arguments: &[v8::Local<v8::Value>],
) -> Option<Result<v8::Local<'s, v8::Value>, ()>> {
    let function = function(scope, exports, name)?;
    let receiver = v8::undefined(scope).into();
    Some(function.call(scope, receiver, arguments).ok_or(()))
}
