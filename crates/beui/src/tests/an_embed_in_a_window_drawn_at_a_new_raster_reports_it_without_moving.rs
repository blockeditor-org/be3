use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{WindowId, WindowReport};

#[test]
fn an_embed_in_a_window_drawn_at_a_new_raster_reports_it_without_moving() {
    let _root = crate::reactive::RootScope::new();
    let placed = create_signal(Vec::new());
    let slot = EmbedSlot::new();
    slot.on_place(move |placement| placed.update(|placed| placed.push(placement)));
    let window = WindowId::next();
    let document = build_node(move || {
        view! {
            <Window id={window} size={vec2(150.0, 120.0)}>|_: WindowHandle|
                <Embed slot={slot} />
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let first = placed
        .get_untracked()
        .last()
        .copied()
        .flatten()
        .expect("the embed is placed in its window");
    assert_eq!(
        first.raster, 1.0,
        "a window not yet rastered draws at the document's scale"
    );

    harness.report_window(WindowReport::Rastered {
        window,
        pixels_per_point: 2.5,
    });
    harness.frame(Vec::new());
    assert_eq!(
        placed.get_untracked().last().copied().flatten(),
        Some(EmbedPlacement {
            raster: 2.5,
            ..first
        }),
        "the embed hears its window's new raster, at the place it was"
    );
}
