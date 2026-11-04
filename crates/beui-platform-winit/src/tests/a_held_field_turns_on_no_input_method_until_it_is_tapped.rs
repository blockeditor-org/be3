use super::*;
use beui_types::Keyboard;
use beui_types::Rect;
use beui_types::window::WindowRequests;

#[test]
fn a_held_field_turns_on_no_input_method_until_it_is_tapped() {
    let field = |keyboard| WindowState {
        requests: WindowRequests {
            ime: Some(ImeArea {
                rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 20.0)),
                cursor: Rect::from_min_max(pos2(4.0, 2.0), pos2(5.0, 18.0)),
                text: None,
                keyboard,
            }),
            ..WindowRequests::default()
        },
        ..WindowState::default()
    };
    let none = WindowState::default();
    let held = field(Keyboard::Held);
    let tapped = field(Keyboard::Shown { asks: 1 });
    assert!(ime_request(&held, &none).is_none());
    assert!(matches!(
        ime_request(&tapped, &held),
        Some(ImeRequest::Enable(_))
    ));
    assert!(matches!(
        ime_request(&none, &tapped),
        Some(ImeRequest::Disable)
    ));
    assert!(ime_request(&none, &held).is_none());
}
