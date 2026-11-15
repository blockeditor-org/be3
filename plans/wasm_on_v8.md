# Running wasm on V8 instead of precompiling

Depends on: none.

Every app module and library is precompiled to a `.cwasm` for each target (`:precompiled` in
`crates/be-host/BUCK`). That is one of the slowest actions in a full build, and the `.cwasm` files
are most of what ships (guides/performance.md, Size). The idea: run guests natively on V8, which
compiles fast enough when a module loads that nothing needs precompiling, and delete
precompilation.

Someone has already done this: embedding V8 to run wasm natively works and is fast, and the
module's imports can be bound directly to native functions, with no JavaScript in between. The
work here is to rediscover how, or to follow a reference implementation if the user links one.

Not yet investigated. Find out what V8 would replace and what it would need, how it compares with
wasmtime as it is today, and what it means on each platform, then fill this in.
