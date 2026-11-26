//! A beui program's entry, wherever it runs. [`main!`] makes the program's `main` natively
//! and, in a browser, the `beui_start` and `automate` its page calls. [`run`] shows a
//! document: natively in a window of its own, through winit or Android's activity, with the
//! space and XR layers and beui-host's drive layer stacked as `BEUI_PLATFORM` and
//! `BEUI_LAYERS` ask; in a browser in the page's element, drawn with wgpu on the native core
//! or as the page itself on the HTML core. [`run_program`] runs any `Program` on the native
//! core. Everything else of beui's engine is beui-host's.

use std::error::Error;

use beui::styled::Root;

#[cfg(target_os = "android")]
pub use beui_platform_android::AndroidApp;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub use beui_platform_winit::winit;

#[cfg(not(beui_html))]
pub use beui_host::native::{Program, RunOptions};
#[cfg(beui_html)]
pub use html::RunOptions;

#[cfg(not(target_arch = "wasm32"))]
pub use crate::runner::native_platform;

#[cfg(beui_html)]
mod html;
#[cfg(not(target_arch = "wasm32"))]
mod runner;
#[cfg(not(target_arch = "wasm32"))]
mod saved_preferences;
#[cfg(target_arch = "wasm32")]
pub mod web;

/// Shows the document `root` builds until the program stops: natively that is when its
/// window closes, so `run` returns then; in a browser it returns once the document shows.
pub async fn run(
    options: RunOptions,
    root: impl FnOnce() -> Root + 'static,
) -> Result<(), Box<dyn Error>> {
    #[cfg(not(beui_html))]
    return run_program(options, beui_host::native::build(root)).await;
    #[cfg(beui_html)]
    return html::run(options, root);
}

/// Runs a program of the native core's own, such as be-host's shell, as [`run`] runs a
/// document.
#[cfg(not(beui_html))]
pub async fn run_program(
    options: RunOptions,
    program: impl Program + 'static,
) -> Result<(), Box<dyn Error>> {
    #[cfg(not(target_arch = "wasm32"))]
    return runner::run_program(options, program).await;
    #[cfg(target_arch = "wasm32")]
    return web::run_program(options, program).await;
}

/// Runs `future` to its end on this thread: what [`main!`] runs `main`'s body with, and what
/// a native program started some other way runs [`run_program`] with, such as from
/// Android's `android_main`.
#[cfg(not(target_arch = "wasm32"))]
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    pollster::block_on(future)
}

/// A program's entry, written once for every platform:
///
/// ```ignore
/// beui_host_window::main! {
///     async fn main() -> Result<(), Box<dyn std::error::Error>> {
///         beui_host_window::run(RunOptions::default(), || view! { <App /> }).await
///     }
/// }
/// ```
///
/// Natively this is `main`, which runs the body to its end. In a browser it is
/// `beui_start(element_id)`, which the page calls once the module's memory and threads are
/// bound: [`run`] shows the program in that element, and the promise rejects with what the
/// body returned, named as a [`web::NamedError`] names it. It also exports `automate(words,
/// timeout_milliseconds)`, which answers drive's commands when the page was opened with
/// `?automation`. On Android, where the activity calls the library's `android_main`, the
/// program defines that itself and runs with [`block_on`].
#[macro_export]
macro_rules! main {
    (async fn main() -> $result:ty $body:block) => {
        #[cfg(not(target_arch = "wasm32"))]
        pub fn main() -> $result {
            $crate::block_on(async move $body)
        }

        #[cfg(target_arch = "wasm32")]
        #[$crate::web::wasm_bindgen::prelude::wasm_bindgen(
            wasm_bindgen = $crate::web::wasm_bindgen,
            wasm_bindgen_futures = $crate::web::wasm_bindgen_futures
        )]
        pub async fn beui_start(
            element_id: ::std::string::String,
        ) -> ::std::result::Result<(), $crate::web::wasm_bindgen::JsValue> {
            $crate::web::start(element_id, async move $body).await
        }

        #[cfg(target_arch = "wasm32")]
        #[$crate::web::wasm_bindgen::prelude::wasm_bindgen(
            wasm_bindgen = $crate::web::wasm_bindgen,
            wasm_bindgen_futures = $crate::web::wasm_bindgen_futures
        )]
        pub async fn automate(
            words: ::std::vec::Vec<::std::string::String>,
            timeout_milliseconds: ::std::option::Option<i32>,
        ) -> ::std::result::Result<::std::string::String, $crate::web::wasm_bindgen::JsValue> {
            $crate::web::automate(words, timeout_milliseconds).await
        }
    };
}
