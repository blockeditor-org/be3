use be_sdk_beui::beui::Vec2;

use super::*;

fn left_of(fixture: &Fixture, words: &str) -> f32 {
    let document = fixture.test.document();
    let text = document
        .root()
        .and_then(|root| text_within(&document, root, words))
        .unwrap_or_else(|| panic!("{words:?} is shown"));
    document
        .node_rect(text)
        .unwrap_or_else(|| panic!("{words:?} is laid out"))
        .left()
}

#[test]
fn a_toast_names_its_app_and_lines_its_text_up_with_the_others() {
    let mut fixture = Fixture::new();
    fixture.test.set_size(Vec2::new(1280.0, 800.0));
    fixture.settle();
    fixture.notify(&[
        IncomingNotification {
            image: Some(picture()),
            ..notification(1, "Mail", "Mia Chen", "Lunch at noon?")
        },
        notification(2, "Calendar", "Standup", "In 5 minutes"),
        notification(3, "Updater", "", ""),
        notification(4, "", "", ""),
    ]);
    fixture.test.settle();

    assert!(
        fixture.says("Mail · 12:01"),
        "a toast says which app sent it, and when"
    );
    assert!(fixture.says("Calendar · 12:02"));
    assert_eq!(
        left_of(&fixture, "Mia Chen"),
        left_of(&fixture, "Standup"),
        "a toast with a picture starts its text where one with the icon does"
    );
    assert!(
        fixture.test.shown(&toast(3)) && fixture.says("Updater"),
        "one with nothing to say is titled with its app"
    );
    assert!(
        !fixture.test.shown(&toast(4)),
        "one with nothing at all to show shows no toast"
    );
    fixture.test.snapshot("notification_toasts_name_their_app");

    fixture.notify(&[
        notification(5, "Chat", "Ana", "Running late"),
        notification(6, "Chat", "Ana", "Start without me"),
    ]);
    fixture.test.settle();
    assert!(
        fixture.test.shown("toasts.more"),
        "the stack folds its oldest"
    );
    fixture.test.click("toasts.more.open");
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.notifications.list"),
        "and its count opens the list of notifications"
    );
}
