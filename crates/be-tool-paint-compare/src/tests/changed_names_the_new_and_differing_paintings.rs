use super::{options, painting, put, ran, scratch};

#[test]
fn changed_names_the_new_and_differing_paintings() {
    let directory = scratch("changed");
    let (made, accepted) = (directory.join("made"), directory.join("accepted"));
    put(&made, "same.browser.paint", &painting("<p>same</p>"));
    put(&accepted, "same.browser.paint", &painting("<p>same</p>"));
    put(&made, "moved.browser.paint", &painting("<p>after</p>"));
    put(&accepted, "moved.browser.paint", &painting("<p>before</p>"));
    put(&made, "new.browser.paint", &painting("<p>new</p>"));
    let mut options = options(&directory);
    let file = directory.join("changed.txt");
    options.changed = Some(file.clone());
    ran(&options);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "moved.browser.paint\nnew.browser.paint\n"
    );

    let unchanged = scratch("unchanged");
    put(&unchanged.join("made"), "same.browser.paint", &painting("<p>same</p>"));
    put(&unchanged.join("accepted"), "same.browser.paint", &painting("<p>same</p>"));
    let mut options = options(&unchanged);
    options.changed = Some(unchanged.join("changed.txt"));
    ran(&options);
    assert_eq!(std::fs::read_to_string(unchanged.join("changed.txt")).unwrap(), "");
}
