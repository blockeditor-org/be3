use super::*;
use crate::{BrowserSnapshot, Painting};

#[test]
fn a_painting_file_decodes_to_its_kind() {
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let native = Painting::decode(&triangles(&[red, red, red]).encode().unwrap()).unwrap();
    assert!(matches!(native, Painting::Native(_)));
    assert_eq!(native.frames(), 3);

    let page = BrowserSnapshot::new([390, 844], "<p>Hello</p>");
    let browser = Painting::decode(&page.encode().unwrap()).unwrap();
    assert_eq!(browser, Painting::Browser(page));
    assert_eq!(browser.frames(), 1);

    let changed = Painting::Native(triangles(&[red, blue, red]));
    assert!(native.same_frame(&changed, 0));
    assert!(!native.same_frame(&changed, 1));
    assert!(!native.same_frame(&changed, 3));
    assert!(browser.same_frame(&browser, 0));
    assert!(!browser.same_frame(&browser, 1));
    assert!(!native.same_frame(&browser, 0));

    assert!(Painting::decode(b"BE3BROWSER 1\n{}").is_err());
    assert_eq!(
        Painting::decode(b"").unwrap_err(),
        "not a paint snapshot"
    );
}
