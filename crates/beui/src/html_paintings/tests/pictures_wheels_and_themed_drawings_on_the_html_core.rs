use super::*;
use beui_macros::view;
use std::rc::Rc;

fn gradient() -> crate::Image {
    let pixels = (0..48 * 32)
        .flat_map(|index| {
            let (x, y) = (index % 48, index / 48);
            [(x * 5) as u8, (y * 7) as u8, 200, 255]
        })
        .collect();
    crate::Image::from_rgba(48, 32, pixels)
}

#[test]
fn pictures_wheels_and_themed_drawings_on_the_html_core() {
    painting([700, 250], || {
        let paint = create_signal(crate::Color32::from_rgb(0x30, 0xA4, 0x6C));
        let theme = use_theme();
        let draw = Prop::dynamic(move || -> Draw {
            let (accent, text) = (theme.accent.get(), theme.text.get());
            Rc::new(move |painter: &Painter, rect: crate::Rect| {
                let centre = rect.center();
                painter.fill_path(
                    &crate::Path::ellipse(centre, crate::vec2(40.0, 28.0)),
                    crate::FillRule::NonZero,
                    accent,
                );
                painter.stroke_path(
                    &crate::Path::ellipse(centre, crate::vec2(40.0, 28.0)),
                    &crate::Stroke::new(2.0).dashed([6.0, 4.0], 0.0),
                    text,
                );
            })
        });
        view! {
            <Root>
                <Frame padding={Padding::all(Space::Lg)}>
                    <List direction={crate::Direction::Horizontal} spacing=Space::Lg>
                        <List spacing=Space::Md>
                            <Picture image={Some(gradient())} radius=Radius::Md />
                            <Drawing draw size={Some(crate::vec2(100.0, 70.0))} />
                        </List>
                        <ColorWheel value={paint} />
                        <OklchColorWheel value={paint} />
                    </List>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_styled_pictures_wheels_and_drawings.browser");
}
