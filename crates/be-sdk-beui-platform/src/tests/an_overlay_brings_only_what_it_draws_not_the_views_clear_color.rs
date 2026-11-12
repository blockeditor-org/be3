use beui::{LayerPolicy, OverlayAnchor, Placement};

use super::*;

const CARD: f32 = 60.0;

#[test]
fn an_overlay_brings_only_what_it_draws_not_the_views_clear_color() {
    let _root = RootScope::new();
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            view! {
                <styled::Layers>
                    <styled::Frame />
                    <styled::Overlay
                        anchor={OverlayAnchor::Rect(Rect::from_min_size(
                            pos2(20.0, 20.0),
                            vec2(160.0, 100.0),
                        ))}
                        placement=Placement::InsideBottom
                        policy=LayerPolicy::Inert
                        open=true
                    >
                        <styled::Frame
                            width={styled::Size::Custom(CARD)}
                            height={styled::Size::Custom(CARD)}
                            fill=styled::ColorRole::Surface
                            radius=styled::Radius::Lg
                        />
                    </styled::Overlay>
                </styled::Layers>
            }
        })
        .reporting(),
    );
    let shown = whole(200.0, 140.0);
    let mut frame = one(&mut surface, &shown);
    for _ in 0..3 {
        frame = one(&mut surface, &shown);
    }
    let [card] = frame.floating_painted.as_slice() else {
        panic!(
            "the open overlay paints one rect: {:?}",
            frame.floating_painted
        );
    };
    assert!(
        frame.backdrop.is_some(),
        "the view clears to an opaque color, which the host backs it with"
    );
    let painting = surface.painting(shown.id()).expect("the view is painted");
    let pixels =
        beui_core::painting::rasterize(&painting.frame, painting.frame.background, |key| {
            painting.textures.get(&key)
        })
        .expect("the painting rasterizes");
    let alpha = |at: Pos2| {
        let [width, _] = pixels.size;
        pixels.rgba[at.y as usize * width as usize + at.x as usize][3]
    };
    assert_eq!(
        alpha(pos2(5.0, 5.0)),
        u8::MAX,
        "away from the overlay the view keeps its clear color"
    );
    assert_eq!(alpha(card.center()), u8::MAX, "the overlay's card is drawn");
    assert_eq!(
        alpha(card.min + vec2(0.5, 0.5)),
        0,
        "outside the card's rounded corner the overlay brings nothing, so the host shows the \
         windows and children under it rather than the view's clear color"
    );
}
