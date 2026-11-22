use beui::datetime::Date;
use beui::reactive::view;
use beui_demo::DemoRoot;
use beui_host_window::RunOptions;

beui_host_window::main! {
    async fn main() -> Result<(), Box<dyn std::error::Error>> {
        beui_host_window::run(RunOptions::default().saving_preferences("beui-demo"), || {
            view! {
                <DemoRoot today={Date::today()} />
            }
        })
        .await
    }
}
