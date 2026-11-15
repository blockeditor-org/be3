use super::*;

/// A message removed outright, which no deletion records, leaves a quiet line in its place: the
/// next message names it, so the chat says one is missing without saying who removed it.
#[test]
fn a_message_removed_outright_shows_as_missing_in_its_place() {
    let mut chat = Harness::new(&[
        (ROSA, "Morning! Is the release still on for today?"),
        (ME, "It is. I'm writing the notes now."),
        (ROSA, "Great, I'll check the installers once they're up."),
    ]);
    let (first, gone, last) = (chat.id(0), chat.id(1), chat.id(2));

    chat.arrive([be_sdk_beui::be_block_content::be_block_model::Change::remove(gone)].into_iter().collect());
    assert_eq!(
        chat.editor.label(&format!("chat.missing.{gone}")),
        "A message that was here is no longer in the chat"
    );
    assert_eq!(
        chat.row(first),
        "Member 7e1a 09:00 Morning! Is the release still on for today?"
    );
    assert!(chat.row(last).contains("Great, I'll check the installers"));
    chat.snapshot("a_message_removed_outright_shows_as_missing_in_its_place");
}
