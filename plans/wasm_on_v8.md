# Running wasm on V8 instead of precompiling

Depends on: none.

Every app module and library is precompiled to a `.cwasm` for each target (`:precompiled` in
`crates/be-host/BUCK`). That is one of the slowest actions in a full build, and the `.cwasm` files
are most of what ships (guides/performance.md, Size). The idea: run guests natively on V8, which
compiles fast enough when a module loads that nothing needs precompiling, and delete
precompilation.

Not yet investigated. Find out what V8 would replace and what it would need, how it compares with
wasmtime as it is today, and what it means on each platform, then fill this in.
