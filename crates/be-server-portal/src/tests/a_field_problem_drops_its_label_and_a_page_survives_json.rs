use super::*;

#[test]
fn a_field_problem_drops_its_label_and_a_page_survives_json() {
    let name = Problem::from_message("Display name: enter a name of at most 200 bytes.");
    assert_eq!(name.field, Some(Field::DisplayName));
    assert_eq!(name.message, "Enter a name of at most 200 bytes.");
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
        problem: Some(name),
        ..Page::default()
    };
    let json = page.to_json();
    assert!(!json.contains('<'));
    assert_eq!(Page::from_json(&json).unwrap(), page);
    assert_eq!(
        page.problem_for(Field::DisplayName),
        "Enter a name of at most 200 bytes."
    );
    assert_eq!(page.general_problem(), "");
}
