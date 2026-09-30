use super::*;

#[test]
fn an_added_or_removed_painting_says_so() {
    let red = [255, 0, 0, 255];
    let added = crate::Change {
        name: "editor.new".to_owned(),
        before: None,
        after: Some(triangles(&[red, red])),
    };
    assert_eq!(added.reasons(), ["was added, 2 frames"]);

    let removed = crate::Change {
        name: "editor.old".to_owned(),
        before: Some(triangle(red)),
        after: None,
    };
    assert_eq!(removed.reasons(), ["was removed"]);
}
