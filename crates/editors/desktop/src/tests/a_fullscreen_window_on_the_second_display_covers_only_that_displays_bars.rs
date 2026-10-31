use block_editor_beui::beui::{LayerLevel, pos2, vec2};

use super::*;

fn levels_on(fixture: &Fixture, test_id: &str, on: Rect) -> Vec<LayerLevel> {
    let found: Vec<(NodeId, LayerLevel)> = {
        let document = fixture.test.document();
        let layers = document.layers();
        document
            .find_test_ids(test_id)
            .into_iter()
            .filter_map(|node| {
                let layer = document.layer_of(&layers, node)?;
                Some((node, layers[layer].level))
            })
            .collect()
    };
    found
        .into_iter()
        .filter(|(node, _)| {
            fixture
                .test
                .desk_rect(*node)
                .is_some_and(|rect| on.contains_rect(rect))
        })
        .map(|(_, level)| level)
        .collect()
}

#[test]
fn a_fullscreen_window_on_the_second_display_covers_only_that_displays_bars() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let right = Rect::from_min_size(pos2(800.0, 0.0), vec2(800.0, 600.0));
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.on_two_displays((left, 1.0), (right, 1.0));
    fixture.display_bar_menu(0, STATUS_BAR, left, &["over.bottom"]);
    fixture.display_bar_menu(1, STATUS_BAR, right, &["over.bottom"]);
    assert_eq!(
        (
            levels_on(&fixture, "desktop.bar.status", left),
            levels_on(&fixture, "desktop.bar.status", right),
        ),
        (vec![LayerLevel::Raised], vec![LayerLevel::Raised]),
        "both Status bars float over the windows"
    );

    fixture.show_windows(&[HostWindow {
        fullscreen: Some(right),
        ..host_window(1, true)
    }]);
    assert_eq!(
        levels_on(&fixture, "desktop.bar.status", right),
        [LayerLevel::Normal],
        "the second display's bar drops beneath the window fullscreen on it"
    );
    assert_eq!(
        levels_on(&fixture, "desktop.bar.status", left),
        [LayerLevel::Raised],
        "while the first display's bar stays over its windows"
    );

    fixture.show_windows(&[HostWindow {
        fullscreen: Some(left),
        ..host_window(1, true)
    }]);
    assert_eq!(
        (
            levels_on(&fixture, "desktop.bar.status", left),
            levels_on(&fixture, "desktop.bar.status", right),
        ),
        (vec![LayerLevel::Normal], vec![LayerLevel::Raised]),
        "and the other way round once the window is fullscreen on the first"
    );
}
