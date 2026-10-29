use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{WindowId, WindowReport};

#[test]
fn an_embed_that_appears_in_a_rastered_window_is_placed_at_its_raster() {
    let _root = crate::reactive::RootScope::new();
    let placed = create_signal(Vec::new());
    let shown = create_signal(false);
    let slot = EmbedSlot::new();
    slot.on_place(move |placement| placed.update(|placed| placed.push(placement)));
    let window = WindowId::next();
    let document = build_node(move || {
        view! {
            <Window id={window} size={vec2(150.0, 120.0)}>|_: WindowHandle|
                <Layers>
                    <Show condition={shown}>||
                        <Embed slot={slot} />
                    </Show>
                </Layers>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.report_window(WindowReport::Rastered {
        window,
        pixels_per_point: 2.5,
    });
    harness.frame(Vec::new());
    shown.set(true);
    harness.report_window(WindowReport::Resized {
        window,
        size: vec2(160.0, 120.0),
    });
    harness.frame(Vec::new());
    let first = placed
        .get_untracked()
        .first()
        .copied()
        .flatten()
        .expect("the embed is placed in its window");
    assert_eq!(
        first.raster, 2.5,
        "laid out with its window, the embed is placed at that window's raster"
    );
}
