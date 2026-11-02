use super::*;

#[test]
fn installing_lists_the_login_session_once_where_greetds_greeters_look() {
    let link = Path::new("/usr/local/bin/be-host");
    let listing = greetd_listing("sway_hrdl\nsway", link).unwrap();
    assert_eq!(
        listing,
        GreetdListing {
            updated: Some(
                "sway_hrdl\nsway\nenv BEUI_PLATFORM=drm /usr/local/bin/be-host --desktop\n"
                    .to_owned()
            ),
            line: "env BEUI_PLATFORM=drm /usr/local/bin/be-host --desktop".to_owned(),
            added: true,
        },
        "the other sessions stay and be-host's is listed once"
    );
    let again = greetd_listing(listing.updated.as_deref().unwrap(), link).unwrap();
    assert_eq!(again.updated, None, "a second install leaves the file as it is");
    assert!(!again.added);

    let spaced = Path::new("/opt/my apps/bin/be-host");
    assert_eq!(
        greetd_listing("sway\n", spaced),
        None,
        "greeters split a line at spaces, so a path with one is not listed"
    );
}
