use super::*;

#[test]
fn locales_are_tried_from_the_most_specific() {
    assert_eq!(
        locales("sr_RS.UTF-8@latin"),
        ["sr_RS@latin", "sr_RS", "sr@latin", "sr"]
    );
    assert_eq!(locales("de_DE"), ["de_DE", "de"]);
    assert_eq!(locales("fr"), ["fr"]);
    assert!(locales("C.UTF-8").is_empty());
    assert!(locales("").is_empty());
}
