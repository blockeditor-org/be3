use super::*;

#[test]
fn a_second_run_that_agrees_but_fails_is_reported_as_flaky() {
    let directory = scratch("again-failed");
    let record = directory.join("record");
    put(&record, "status", b"101\n");
    put(
        &record.join("made"),
        "same.browser.paint",
        &painting("<p>same</p>"),
    );
    put(
        &directory.join("first"),
        "same.browser.paint",
        &painting("<p>same</p>"),
    );
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
