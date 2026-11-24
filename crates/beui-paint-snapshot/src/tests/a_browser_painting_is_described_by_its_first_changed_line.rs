use super::*;
use crate::{BrowserSnapshot, Painting};

#[test]
fn a_browser_painting_is_described_by_its_first_changed_line() {
    let hello = BrowserSnapshot::new([390, 844], "<p>Hello</p>");
    let goodbye = BrowserSnapshot::new([390, 844], "<p>Goodbye</p>");
    let before = Painting::Browser(hello.clone());
    let after = Painting::Browser(goodbye.clone());

    assert!(before.difference(&before.clone()).is_none());
    let difference = before.difference(&after).unwrap();
    assert_eq!(difference.frame, Some(0));
    assert!(
        difference
            .description
            .starts_with("browser document or embedded files changed at line "),
        "{}",
        difference.description
    );
    assert!(difference.description.contains("Goodbye"));

    let (accepted, made) = (hello.encode().unwrap(), goodbye.encode().unwrap());
    assert_eq!(crate::changed(&accepted, &accepted), None);
    assert_eq!(crate::changed(&accepted, &made), Some(difference.description));

    let native = triangle([255, 0, 0, 255]);
    assert_eq!(
        crate::changed(&native.encode().unwrap(), &made).unwrap(),
        "the painting became a browser painting"
    );
    assert_eq!(
        Painting::Browser(hello)
            .difference(&Painting::Native(native))
            .unwrap()
            .description,
        "the browser painting became a native painting"
    );
}
