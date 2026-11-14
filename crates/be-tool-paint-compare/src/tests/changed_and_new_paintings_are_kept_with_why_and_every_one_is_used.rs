use super::{options, painting, put, ran, scratch};

#[test]
fn changed_and_new_paintings_are_kept_with_why_and_every_one_is_used() {
    let directory = scratch("update");
    let (made, accepted) = (directory.join("made"), directory.join("accepted"));
    put(&made, "same.browser.paint", &painting("<p>same</p>"));
    put(&accepted, "same.browser.paint", &painting("<p>same</p>"));
    put(&made, "moved.browser.paint", &painting("<p>after</p>"));
    put(&accepted, "moved.browser.paint", &painting("<p>before</p>"));
    put(&made, "new.browser.paint", &painting("<p>new</p>"));
    let options = options(&directory);
    ran(&options);
    let out = &options.out;
    let changed = |name: &str| out.join("changed").join(name);
    assert!(!changed("same.browser.paint").exists());
    assert_eq!(
        std::fs::read(changed("moved.browser.paint")).unwrap(),
        painting("<p>after</p>")
    );
    assert!(
        std::fs::read_to_string(changed("moved.browser.paint.why"))
            .unwrap()
            .contains("+ ")
    );
    assert_eq!(
        std::fs::read_to_string(changed("new.browser.paint.why")).unwrap(),
        "there is no accepted painting"
    );
    let used = |name: &str| std::fs::read_to_string(out.join("used").join(name)).unwrap();
    assert_eq!(
        used("same.browser.paint"),
        super::super::sha256(&painting("<p>same</p>"))
    );
    assert_eq!(used("new.browser.paint"), super::super::sha256(b""));
}
