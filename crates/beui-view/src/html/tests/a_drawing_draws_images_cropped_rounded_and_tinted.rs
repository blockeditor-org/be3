use super::*;
use std::rc::Rc;

fn checkers() -> Image {
    let pixels = (0..8 * 8)
        .flat_map(|index| match (index % 8 + index / 8) % 2 {
            0 => [70, 110, 220, 255],
            _ => [250, 250, 247, 255],
        })
        .collect();
    Image::from_rgba(8, 8, pixels)
}

#[test]
fn a_drawing_draws_images_cropped_rounded_and_tinted() {
    painting([300, 100], || {
        let image = checkers();
        let draw: Draw = Rc::new(move |painter: &Painter, _rect: Rect| {
            let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            let at = |x: f32| Rect::from_min_size(pos2(x, 0.0), vec2(64.0, 64.0));
            painter.image(at(0.0), whole, &image, Color::WHITE, 0.0, false);
            painter.image(at(70.0), whole, &image, Color::WHITE, 16.0, true);
            let quarter = Rect::from_min_max(pos2(0.0, 0.0), pos2(0.5, 0.5));
            painter.image(at(140.0), quarter, &image, Color::WHITE, 0.0, false);
            painter.image(at(210.0), whole, &image, AMBER, 0.0, false);
        });
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START} padding={Sides::all(10.0)}>
                    <Drawing draw={draw} size={Some(vec2(274.0, 64.0))} />
                </Frame>
            </Root>
        }
    })
    .snapshot("html_drawing_images.browser");
}
