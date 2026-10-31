use beui::{LayerPolicy, OverlayAnchor, Placement};
use block_editor_plugin::{AnchorSpace, HostTransform};

use super::*;

const ORIGIN: Pos2 = Pos2::new(200.0, 100.0);

#[test]
fn an_anchored_view_left_empty_keeps_a_finite_place() {
    let _root = RootScope::new();
    let filled = create_signal(true);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            let shown = create_memo(move || filled.get());
            view! {
                <styled::Overlay
                    anchor={OverlayAnchor::Point(pos2(30.0, 20.0))}
                    placement={Placement::Over(0)}
                    policy=LayerPolicy::Pass
                    open=true
                >
                    <styled::Layers>
                        <Show condition={shown}>||
                            <styled::Button label="Row" on_click={|| {}} />
                        </Show>
                    </styled::Layers>
                </styled::Overlay>
            }
        }),
    );
    let mut shown = whole(100.0, 60.0);
    shown.anchor_space = Some(AnchorSpace {
        to_host: HostTransform {
            origin: ORIGIN,
            scale: Vec2::splat(1.0),
        },
        bounds: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
    });
    for _ in 0..3 {
        one(&mut surface, &shown);
    }
    let frame = one(&mut surface, &shown);
    let [layer] = frame.anchored.as_slice() else {
        panic!("the open overlay is one layer: {:?}", frame.anchored);
    };
    assert!(layer.placement.rect.is_positive());

    filled.set(false);
    for _ in 0..3 {
        one(&mut surface, &shown);
    }
    let frame = one(&mut surface, &shown);
    let [layer] = frame.anchored.as_slice() else {
        panic!(
            "the emptied overlay is still one layer: {:?}",
            frame.anchored
        );
    };
    let rect = layer.placement.rect;
    assert!(
        [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
            .iter()
            .all(|side| side.is_finite()),
        "with nothing left to paint, the layer still has a place the host can take input at: {rect:?}"
    );
}
