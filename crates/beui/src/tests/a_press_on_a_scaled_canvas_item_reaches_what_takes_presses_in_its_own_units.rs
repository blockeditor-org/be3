use super::*;
use crate::input::PointerPress;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_press_on_a_scaled_canvas_item_reaches_what_takes_presses_in_its_own_units() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<PointerPress>::new());
    let under = create_signal(0_usize);
    let button = NodeRef::new();
    let drawing = NodeRef::new();
    let passing = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Canvas view={Some(CanvasView::new(pos2(20.0, 10.0), 2.0))}>
                <CanvasItem x=0.0 y=0.0 width=300.0 height=200.0>
                    <Interactive on_click={move || under.update(|count| *count += 1)}>
                        <Frame width=300.0 height=200.0 />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=10.0 y=10.0 width=40.0 height=20.0 scales=true>
                    <Interactive
                        @node_ref=&button
                        hit_at={|local: Pos2| local.x < 30.0}
                        on_click_at={move |press: PointerPress| heard.update(|heard| heard.push(press))}
                    >
                        <Frame width=40.0 height=20.0 />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=10.0 y=50.0 width=40.0 height=20.0 scales=true>
                    <Frame @node_ref=&drawing width=40.0 height=20.0 fill={Color32::from_gray(90)} />
                </CanvasItem>
                <CanvasItem x=10.0 y=90.0 width=40.0 height=20.0 scales=true pointer=false>
                    <Interactive
                        @node_ref=&passing
                        on_click={move || heard.update(|heard| heard.clear())}
                    >
                        <Frame width=40.0 height=20.0 />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::sized(document, vec2(800.0, 600.0));
    harness.frame(Vec::new());

    let rect = harness.rect(button.get());
    assert_eq!(rect.size(), vec2(80.0, 40.0), "the item is shown at the zoom");
    let at = rect.min + vec2(10.0, 6.0);
    harness.click(at);
    let pressed = heard.get_untracked();
    assert_eq!(pressed.len(), 1, "a press on a scaled item reaches it");
    assert_eq!(pressed[0].pos, at, "pos stays in the window's coordinates");
    assert_eq!(
        (pressed[0].local, pressed[0].size),
        (pos2(5.0, 3.0), vec2(40.0, 20.0)),
        "local and size are in the item's own units"
    );
    assert_eq!(under.get_untracked(), 0, "and it goes no further");

    harness.click(rect.min + vec2(70.0, 6.0));
    assert_eq!(
        (heard.get_untracked().len(), under.get_untracked()),
        (1, 1),
        "its own hit test reads its own units, so a press past 30 of them falls through"
    );

    harness.click(harness.rect(drawing.get()).center());
    assert_eq!(
        under.get_untracked(),
        2,
        "a scaled item that only draws lets presses through"
    );

    harness.click(harness.rect(passing.get()).center());
    assert_eq!(
        (heard.get_untracked().len(), under.get_untracked()),
        (1, 3),
        "and so does one that takes no pointer"
    );
}
