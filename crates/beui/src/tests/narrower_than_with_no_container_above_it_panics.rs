use super::build_node;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::narrower_than;

#[test]
#[should_panic(
    expected = "`narrower_than` answers for the nearest `Container` above it, and there is none"
)]
fn narrower_than_with_no_container_above_it_panics() {
    let _root = crate::reactive::RootScope::new();
    build_node(move || {
        let _narrow = narrower_than(300.0);
        view! {
            <Spacer />
        }
    });
}
