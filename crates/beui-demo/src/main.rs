#[cfg(not(target_arch = "wasm32"))]
use beui::datetime::Date;
#[cfg(not(target_arch = "wasm32"))]
use beui::reactive::view;
#[cfg(not(target_arch = "wasm32"))]
use beui_demo::DemoRoot;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui_host::native::run_with(
        beui_host::native::RunOptions::default().saving_preferences("beui-demo"),
        beui_host::native::build(|| {
            view! {
                <DemoRoot today={Date::today()} />
            }
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
