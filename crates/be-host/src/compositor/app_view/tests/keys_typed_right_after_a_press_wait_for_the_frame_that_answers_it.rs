use super::*;

#[test]
fn keys_typed_right_after_a_press_wait_for_the_frame_that_answers_it() {
    assert_eq!(
        deck(false, app(3, 3), vec![press()]).len(),
        1,
        "the press goes through"
    );
    held_keys::sent_press(&deck_view(), 3);
    assert_eq!(
        deck(false, app(4, 3), vec![letter("T")]),
        [],
        "a key typed before the deck drew the frame that took the press waits for it"
    );
    assert_eq!(
        deck(true, app(4, 4), vec![letter("h")]),
        [],
        "that frame asked to give the keyboard to the notes, so the keys are theirs"
    );
    held_keys::given(DECK, notes_view(), false);
    let mut notes = ForwardedInput {
        events: Vec::new(),
        rect: Rect::ZERO,
        hovered: false,
        focused: true,
        pointer: None,
        modifiers: beui::Modifiers::NONE,
    };
    let typing = App {
        frames: (1, 1),
        content: true,
        typing: true,
    };
    held_keys::route(&notes_view(), false, typing, &mut notes);
    assert_eq!(notes.events, [letter("T"), letter("h")]);

    assert_eq!(deck(false, app(5, 5), vec![press()]).len(), 1);
    held_keys::sent_press(&deck_view(), 5);
    assert_eq!(deck(false, app(6, 5), vec![letter("x")]), []);
    assert_eq!(
        held_keys::owed_now(&deck_view(), false, app(6, 6), true),
        Some(beui::Modifiers::NONE),
        "a press whose frame asked for nothing gives the deck its keys as soon as it is drawn"
    );
    assert_eq!(deck(false, app(6, 6), Vec::new()), [letter("x")]);
}
