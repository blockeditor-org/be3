use super::{options, painting, put, ran, scratch};

#[test]
fn a_record_without_update_fails_on_a_changed_painting() {
    let directory = scratch("record");
    let record = directory.join("record");
    put(&record, "log", b"test output\n");
    put(&record, "status", b"0\n");
    put(
        &directory.join("made"),
        "moved.browser.paint",
        &painting("<p>after</p>"),
    );
    put(
        &directory.join("accepted"),
        "moved.browser.paint",
        &painting("<p>before</p>"),
    );
    let mut options = options(&directory);
    options.record = Some(record);
    options.update = false;
    ran(&options);
    let out = &options.out;
    assert_eq!(std::fs::read_to_string(out.join("status")).unwrap(), "1\n");
    let log = std::fs::read_to_string(out.join("log")).unwrap();
    assert!(log.starts_with("test output\nthe painting moved.browser.paint changed: "));
    assert!(!out.join("changed/moved.browser.paint").exists());
}
