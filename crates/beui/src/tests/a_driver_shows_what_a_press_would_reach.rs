use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;

#[component]
fn Wrapper() -> NodeId {
    view! {
        <Interactive @test_id={"wrapper"}>
            <Frame width=200.0 height=100.0 />
        </Interactive>
    }
}

#[test]
fn a_driver_shows_what_a_press_would_reach() {
    let document = build_node(|| {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=200.0 height=100.0>
                    <Wrapper />
                </CanvasItem>
                <CanvasItem x=0.0 y=150.0 width=120.0 height=40.0>
                    <Button
                        label="Save"
                        variant=ButtonVariant::Primary
                        @test_id={"save"}
                        on_click={|| {}}
                    />
                </CanvasItem>
            </Canvas>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));

    let wrapped = driven.ask(&["hit", "50,50"]).expect("the driver hit tests");
    assert!(
        wrapped.contains("#wrapper in Interactive in Wrapper")
            && wrapped.contains("handling nothing"),
        "a press on the wrapper names it, the component that made it, and that it handles nothing:\n{wrapped}"
    );

    let button = driven.ask(&["hit", "#save"]).expect("the driver hit tests");
    assert!(
        button.contains("handling click"),
        "a press on the button reaches something that handles a click:\n{button}"
    );

    let empty = driven
        .ask(&["hit", "300,300"])
        .expect("the driver hit tests");
    assert_eq!(empty, "no Interactive\n");
}
