use beui::datetime::Date;
use beui::reactive::view;
use beui_demo::DemoRoot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui_host_window::run_with(
        beui_host::native::RunOptions::default().saving_preferences("beui-demo"),
        beui_host::native::build(|| {
            view! {
                <DemoRoot today={Date::today()} />
            }
        }),
    )
}
