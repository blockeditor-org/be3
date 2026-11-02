use super::*;

#[test]
fn the_apps_button_is_named_apps() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let document = fixture.test.document();
    let mut names = Vec::new();
    let mut pending = vec![document.find_test_id("desktop.apps").expect("the bar has an Apps button")];
    while let Some(node) = pending.pop() {
        if let Some(label) = document.accessibility.get(&node).and_then(|node| node.label()) {
            names.push(label.to_owned());
        }
        pending.extend(document.children(node));
    }
    assert_eq!(
        names,
        ["Apps"],
        "the button that opens the apps is named for them, not for the code behind it"
    );
}
