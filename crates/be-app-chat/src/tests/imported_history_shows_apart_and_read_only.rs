use super::*;

/// History an import brought shows apart from the chat's own messages, under who imported it,
/// read-only, with no verdict on when it arrived: its times are the file's word.
#[test]
fn imported_history_shows_apart_and_read_only() {
    let chat = Harness::new(&[]);
    let store = chat.editor.store();
    let old = Uuid::from_u128(0x01d0);
    for (place, body) in [(7, "Minutes from the kickoff"), (8, "Venue is booked")] {
        store.log_imported(
            None,
            ROSA,
            message(MORNING - 3 * 24 * 60 * MINUTE, body).encode(),
            ME,
            (old, place),
        );
    }
    store.log_entry(
        None,
        ROSA,
        message(MORNING, "Welcome to the new chat").encode(),
        Some(MORNING),
    );
    let mut chat = chat.reopened();
    chat.run();

    assert_eq!(
        chat.editor.label("chat.history"),
        "Imported by You from an export"
    );
    assert!(chat.row("history.7").contains("Minutes from the kickoff"));
    assert!(
        !chat.editor.shown("chat.message.history.7.reply"),
        "imported history takes no reply"
    );
    assert!(!chat.editor.shown("chat.message.history.7.react"));
    assert!(!chat.editor.shown("chat.message.history.8.arrived"));
    assert_eq!(chat.bodies(), ["Welcome to the new chat"]);
    chat.snapshot("imported_history_shows_apart_and_read_only");
}
