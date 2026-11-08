# App libraries: what is left

Depends on: none.

Apps link any set of shared libraries (guides/app_libraries.md): Rust apps link base (`//crates/be-sdk-library-base:module`, the fixed-address root) and beui (`//crates/be-sdk-library-beui:module`), and an app may link none. The framework-neutral editor state is in be-sdk (`editor_state`, and the region driver `be_sdk::driver`); beui's signals over it are in be-sdk-beui. What remains:

## Docks and overlays below beui

`be-sdk-beui`'s `docked.rs` is built on `beui::dock` and `beui::dock_host`, and the floats, scrims and window requests in the `Frame` accounting (anchored views, floated windows and scrims in be-sdk-beui-platform) are the overlay API. Both were still changing, so they stayed where they are. Move them below beui once they settle, in the same change that would let a second framework use them. `related_content.rs` and the memo-based walks in `editor/settings.rs` and `editor/tags.rs` also stayed in be-sdk-beui.

## External textures

External textures and external image copies are core WebGPU, but need a design for what the source is (a host video frame, a camera, a decoded image) that keeps the texture on the GPU. **(ask)** before designing it; until then `be-wasm-gpu-guest` says they are not available to apps yet.

## Later

- An egui (or other framework's) adapter, as one new crate on top of base: every feature in the docking and overlay APIs would have to be built twice while they are still changing, so wait for them to settle.
- Third-party apps, and publishing libraries for them to build against: after the app APIs stabilize, the interface is hardened and permission prompts exist.
- Removing the fixed-address root (decisions/app_library_base_is_a_fixed_address_root.md) needs `-Zbuild-std` and a threaded wasi-libc built with `-fPIC`.
