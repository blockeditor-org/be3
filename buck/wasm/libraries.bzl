# The libraries a linked wasm module needs (guides/app_libraries.md): every
# library module in its closure, in dependency order, the root first. A
# module's rule gives it, and the app stages each library under its identity
# (crates/be-tool-buck's stage).
WasmLibrariesInfo = provider(
    fields = {
        "closure": provider_field(list, default = []),
    }
)

# An app's worker modules (guides/adding_an_app.md, Workers): what its
# :module carries for the program to stage under workers/<app id>/, each
# by its file name.
AppWorkersInfo = provider(
    fields = {
        "modules": provider_field(list, default = []),
    }
)
