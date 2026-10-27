#[cfg(not(target_arch = "wasm32"))]
use beui::datetime::Date;
#[cfg(not(target_arch = "wasm32"))]
use beui::reactive::view;
#[cfg(not(target_arch = "wasm32"))]
use beui_demo::DemoRoot;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::native::run_with(
        beui::native::RunOptions::default().saving_preferences("beui-demo"),
        beui::native::build(|| {
            view! {
                <DemoRoot today={Date::today()} />
            }
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
