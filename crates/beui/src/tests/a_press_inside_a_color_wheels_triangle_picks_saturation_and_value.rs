use super::*;
use crate::color::Hsva;
use crate::native::testing::{Showing, Stepped, Target};
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use beui_core::window::WindowSize;

#[test]
fn a_press_inside_a_color_wheels_triangle_picks_saturation_and_value() {
    let _root = RootScope::new();
    let changes = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorWheel
                    @test_id={"wheel"}
                    value={Binding::new(Hsva::new(200.0, 1.0, 1.0, 1.0).to_color(), move |next| changes.update(|changes| changes.push(next)))}
                />
            </List>
        }
    });
    let window = WindowSize {
        size: TALL_VIEWPORT,
        scale: 1.0,
    };
    let mut app = Stepped::new(context(), Showing(document), window, None);
    app.run();

    app.click(Target::id("wheel").at(vec2(WHEEL_SIZE / 2.0, WHEEL_SIZE / 2.0)));

    let changes = changes.get_untracked();
    assert_eq!(changes.len(), 1);
    let picked = Hsva::from_color(changes[0]);
    assert!((picked.hue - 200.0).abs() < 1.0, "{picked:?}");
    assert!((picked.saturation - 0.5).abs() < 0.02, "{picked:?}");
    assert!((picked.value - 2.0 / 3.0).abs() < 0.02, "{picked:?}");
}
