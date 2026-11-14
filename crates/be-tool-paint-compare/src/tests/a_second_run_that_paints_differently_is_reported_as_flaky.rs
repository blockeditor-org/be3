use super::{options, painting, put, ran, scratch};

#[test]
fn a_second_run_that_paints_differently_is_reported_as_flaky() {
    let directory = scratch("again");
    let record = directory.join("record");
    put(&record, "log", b"second run\n");
    put(&record, "status", b"0\n");
    let (made, first) = (record.join("made"), directory.join("first"));
    put(&made, "same.browser.paint", &painting("<p>same</p>"));
    put(&first, "same.browser.paint", &painting("<p>same</p>"));
    put(&made, "hover.browser.paint", &painting("<p>plain</p>"));
    put(&first, "hover.browser.paint", &painting("<p>hovered</p>"));
    put(&first, "once.browser.paint", &painting("<p>once</p>"));
    let mut options = options(&directory);
    options.again = true;
    options.made = made;
    options.accepted = first;
    options.record = Some(record);
    ran(&options);
    let out = &options.out;
    let flaky = std::fs::read_to_string(out.join("flaky")).unwrap();
    let lines: Vec<&str> = flaky.lines().collect();
    assert_eq!(lines.len(), 2, "{flaky}");
    assert!(lines[0].starts_with("hover.browser.paint: "), "{flaky}");
    assert_eq!(lines[1], "once.browser.paint: the second run did not make it");
    assert!(out.join("changed/hover.browser.paint").exists());
    assert!(!out.join("changed/same.browser.paint").exists());
    assert_eq!(std::fs::read(out.join("log")).unwrap(), b"second run\n");
}

#[test]
fn a_second_run_that_agrees_but_fails_is_reported_as_flaky() {
    let directory = scratch("again-failed");
    let record = directory.join("record");
    put(&record, "status", b"101\n");
    put(&record.join("made"), "same.browser.paint", &painting("<p>same</p>"));
    put(&directory.join("first"), "same.browser.paint", &painting("<p>same</p>"));
    let mut options = options(&directory);
    options.again = true;
    options.made = record.join("made");
    options.accepted = directory.join("first");
    options.record = Some(record);
    ran(&options);
    assert_eq!(
        std::fs::read_to_string(options.out.join("flaky")).unwrap(),
        "the test failed when run again (status 101)\n"
    );
}
