use super::*;
use crate::reactive::{build, component, component_size, view};
use crate::unstyled::MenuItem;

#[component]
fn SizedItem() -> MenuItem {
    let _ = component_size();
    view! {
        <MenuItem label="sized" />
    }
}

#[test]
#[should_panic(expected = "returns a value that is no node")]
fn a_value_child_that_watches_its_size_panics_when_built() {
    let _ = build(|| {
        let _ = view! {
            <SizedItem />
        };
        view! {
            <Frame />
        }
    });
}
