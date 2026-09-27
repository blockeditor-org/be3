use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;
use crate::reactive::{Frame, List, build, view};

#[test]
fn two_nodes_on_screen_with_one_test_id_are_an_error() {
    let document = build(|| {
        view! {
            <List spacing=0.0>
                <Frame @test_id="twin" height=10.0 />
                <Frame @test_id="twin" height=20.0 />
                <Frame @test_id="single" height=30.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    assert!(output.test_id_rect("single").is_some());
    assert!(
        catch_unwind(AssertUnwindSafe(|| output.test_id_rect("twin"))).is_err(),
        "a painted id that names two nodes fails when asked for"
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| harness.document().find_test_id("twin"))).is_err(),
        "looking the id up in the document fails the same way"
    );
}
