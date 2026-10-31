# The libraries a linked wasm module needs (guides/plugin_libraries.md): every
# library module in its closure, in dependency order, the root first. A
# module's rule gives it, and the app stages each library under its identity
# (buck/app/stage.sh).
WasmLibrariesInfo = provider(
    fields = {
        "closure": provider_field(list, default = []),
    }
)
