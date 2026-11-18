use super::*;

/// A message redacted by someone other than its author (a manager, the server) is not its
/// author's deletion: it shows in its place, saying so, and takes nothing to do with it.
#[test]
fn a_message_removed_by_someone_else_says_so_in_its_place() {
    let mut chat = Harness::new(&[
        (ROSA, "Morning! Is the release still on for today?"),
        (ME, "It is. I'm writing the notes now."),
        (ROSA, "Great, I'll check the installers once they're up."),
    ]);
    let (first, gone, last) = (chat.seq(0), chat.seq(1), chat.seq(2));
    chat.click(&format!("chat.message.{gone}.reply"));
    assert!(chat.editor.shown("chat.replying"));

    chat.editor.store().redact_entry(None, gone);
    chat.run();
    assert_eq!(
        chat.row(gone),
        "You 09:03 Removed, not by its author",
        "it shows who wrote it, and that someone else removed it"
    );
    assert!(
        !chat.editor.shown(&format!("chat.message.{gone}.reply")),
        "a removed message takes no reply"
    );
    assert!(
        !chat.editor.shown("chat.replying"),
        "the reply to it ends"
    );
    assert!(chat.row(first).contains("Is the release still on"));
    assert!(chat.row(last).contains("Great, I'll check the installers"));
    chat.snapshot("a_message_removed_by_someone_else_says_so_in_its_place");
}
