use super::*;
use crate::reactive::*;

#[test]
fn a_driver_lists_components_and_their_internals() {
    let document = build_node(|| {
        view! {
            <styled::Frame @test_id="frame">
                <styled::Text @test_id="greeting">"Hi"</styled::Text>
            </styled::Frame>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));
    let public = driven
        .ask(&["app", "comps"])
        .expect("the component tree is answered through a frame");
    assert_eq!(
        public,
        "styled::Frame #frame\n  styled::Text #greeting\n    \"Hi\"\n"
    );
    let full = driven.ask(&["app", "comps", "--internals"]).unwrap();
    assert!(
        full.contains("  internals\n    primitives::Frame #frame\n"),
        "{full}"
    );
    assert!(
        full.contains("    internals\n      primitives::Text #greeting\n"),
        "{full}"
    );
    assert!(full.contains("  styled::Text #greeting\n"), "{full}");
    assert!(
        driven
            .ask(&["app", "comps", "--unknown"])
            .unwrap_err()
            .contains("usage: comps")
    );
}
