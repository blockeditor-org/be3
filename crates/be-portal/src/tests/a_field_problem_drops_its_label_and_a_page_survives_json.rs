use super::*;

#[test]
fn a_field_problem_drops_its_label_and_a_page_survives_json() {
    let confirmation = Problem::from_message("Password confirmation: the passwords do not match.");
    assert_eq!(confirmation.field, Some(Field::Confirmation));
    assert_eq!(confirmation.message, "The passwords do not match.");
    let general = Problem::from_message("This page expired; reload it and try again");
    assert_eq!(general.field, None);
    assert_eq!(
        general.message,
        "This page expired; reload it and try again"
    );
    let page = Page {
        screen: Screen::Message {
            title: "</script><script>".into(),
            text: "ok".into(),
        },
        problem: Some(confirmation),
        ..Page::default()
    };
    let json = page.to_json();
    assert!(!json.contains('<'));
    assert_eq!(Page::from_json(&json).unwrap(), page);
    assert_eq!(
        page.problem_for(Field::Confirmation),
        "The passwords do not match."
    );
    assert_eq!(page.general_problem(), "");
}
