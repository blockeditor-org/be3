use std::time::Duration;

use block_editor_beui::beui::{pos2, vec2};
use block_editor_beui::{HostImage, NotificationUrgency};

use super::*;

const PICTURE: u32 = 16;

fn picture() -> HostImage {
    let rgba = (0..PICTURE * PICTURE)
        .flat_map(|at| match (at / PICTURE + at % PICTURE) % 2 {
            0 => [64, 120, 220, 255],
            _ => [240, 240, 255, 255],
        })
        .collect();
    HostImage {
        width: PICTURE,
        height: PICTURE,
        rgba,
    }
}

#[test]
fn a_notification_toast_shows_above_the_bar_until_its_time_is_up() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.notify(&[
        IncomingNotification {
            image: Some(picture()),
            ..notification(1, "Mail", "Mia Chen", "Lunch at noon?")
        },
        IncomingNotification {
            urgency: NotificationUrgency::Critical,
            ..notification(2, "Power", "Battery low", "Plug the laptop in soon.")
        },
    ]);
    fixture.test.settle();
    let bar = fixture.test.rect_of("desktop.bar");
    let card = fixture.test.rect_of(&toast(2));
    assert!(
        card.bottom() <= bar.top(),
        "the toasts stand above the bar rather than over it: {card:?} {bar:?}"
    );
    fixture.test.snapshot("notification_toasts_above_the_bar");

    fixture.test.advance(Duration::from_secs(6));
    fixture.settle();
    assert!(!fixture.test.shown(&toast(1)), "its time is up");
    assert!(
        fixture.test.shown(&toast(2)),
        "a critical one waits to be answered"
    );
    assert!(
        fixture.signals().is_empty(),
        "a toast that hides leaves its notification open"
    );

    let right = Rect::from_min_size(pos2(400.0, 0.0), vec2(400.0, 400.0));
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 600.0));
    fixture
        .test
        .set_monitors(vec![("right", right), ("left", left)]);
    fixture.settle();
    let card = fixture.test.rect_of(&toast(2));
    assert!(
        right.contains_rect(card),
        "the toasts show on the first screen the host reports: {card:?}"
    );
    assert!(
        card.bottom() > right.bottom() - card.height() / 2.0,
        "down to its foot, where the bar does not reach: {card:?}"
    );
}
