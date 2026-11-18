use super::{options, painting, put, ran, scratch};

#[test]
fn names_says_which_paintings_changed() {
    let directory = scratch("changed");
    let (made, accepted) = (directory.join("made"), directory.join("accepted"));
    put(&made, "same.browser.paint", &painting("<p>same</p>"));
    put(&accepted, "same.browser.paint", &painting("<p>same</p>"));
    put(&made, "moved.browser.paint", &painting("<p>after</p>"));
    put(&accepted, "moved.browser.paint", &painting("<p>before</p>"));
    put(&made, "new.browser.paint", &painting("<p>new</p>"));
    let mut first = options(&directory);
    let file = directory.join("changed.txt");
    first.names = Some(file.clone());
    ran(&first);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "changed moved.browser.paint\nchanged new.browser.paint\nsame same.browser.paint\n"
    );

    let unchanged = scratch("unchanged");
    put(
        &unchanged.join("made"),
        "same.browser.paint",
        &painting("<p>same</p>"),
    );
    put(
        &unchanged.join("accepted"),
        "same.browser.paint",
        &painting("<p>same</p>"),
    );
    let mut options = options(&unchanged);
    options.names = Some(unchanged.join("changed.txt"));
    ran(&options);
    assert_eq!(
        std::fs::read_to_string(unchanged.join("changed.txt")).unwrap(),
        "same same.browser.paint\n"
    );
}
