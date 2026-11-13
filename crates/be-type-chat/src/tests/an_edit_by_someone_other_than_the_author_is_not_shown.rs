use super::*;

#[test]
fn an_edit_by_someone_other_than_the_author_is_not_shown() {
    let chat = sent(&ChatContent::default(), ADA, &["I'll bring the cake"]);
    let cake = first(&chat);

    let chat = edited(&chat, [chat.edit(CHAT, cake, BO, 5, "Bo brings the cake")]);

    assert_eq!(bodies(&chat), ["I'll bring the cake"]);
    assert_eq!(chat.edit_of(cake), None);
    assert!(!chat.shown()[0].edited);
}
