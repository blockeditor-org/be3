use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(count: Signal<u32>) -> NodeId {
    view! {
        <Card>
            <Dynamic value={count} fallback={|| view! { <Frame /> }}> //~ ERROR a `fallback` on a `Dynamic` or `Keyed` needs an `Option` value
                |_: u32| <Frame />
            </Dynamic>
        </Card>
    }
}
