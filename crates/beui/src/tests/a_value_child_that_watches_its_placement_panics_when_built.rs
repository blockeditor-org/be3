use super::*;
use crate::reactive::{build, component, component_placed, view};
use crate::unstyled::MenuItem;

#[component]
fn PlacedItem() -> MenuItem {
    let _ = component_placed();
    view! {
        <MenuItem label="placed" />
    }
}

#[test]
#[should_panic(expected = "returns a value that is no node")]
fn a_value_child_that_watches_its_placement_panics_when_built() {
    let _ = build(|| {
        let _ = view! {
            <PlacedItem />
        };
        view! {
            <Frame />
        }
    });
}
