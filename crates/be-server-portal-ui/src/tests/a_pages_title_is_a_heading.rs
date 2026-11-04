use super::*;

#[test]
fn a_pages_title_is_a_heading() {
    for (screen, title) in [
        (Screen::CheckEmail, "Check your email"),
        (Screen::Approved, "Sign-in approved"),
    ] {
        let html = render(&page(screen));
        let heading = html
            .split('<')
            .find(|tag| tag.contains("role=\"heading\""))
            .unwrap_or_else(|| panic!("{title} has no heading: {html}"));
        assert!(heading.contains("aria-level=\"1\""), "<{heading}");
        assert!(heading.ends_with(&format!(">{title}")), "<{heading}");
    }
}
