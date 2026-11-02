use super::*;

#[test]
fn installing_replaces_the_line_an_earlier_install_listed_for_greetd() {
    let link = Path::new("/usr/local/bin/be-host");
    let listing = greetd_listing(
        "sway\n/usr/local/bin/be-host --session\n/opt/bin/be-host --session\n",
        link,
    )
    .unwrap();
    assert_eq!(
        listing.updated.as_deref(),
        Some(
            "sway\n/opt/bin/be-host --session\nenv BEUI_PLATFORM=drm /usr/local/bin/be-host --desktop\n"
        ),
        "the line that runs this path with --session goes, and another path's stays"
    );

    let listing = greetd_listing(
        "env BEUI_PLATFORM=drm /usr/local/bin/be-host --desktop\n/usr/local/bin/be-host --session",
        link,
    )
    .unwrap();
    assert!(!listing.added, "the login session was already listed");
    assert_eq!(
        listing.updated.as_deref(),
        Some("env BEUI_PLATFORM=drm /usr/local/bin/be-host --desktop\n"),
        "but the stale line still goes"
    );
}
