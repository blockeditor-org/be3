use super::*;

#[test]
fn installing_lists_the_login_session_once_where_greetds_greeters_look() {
    let dir = std::env::temp_dir().join(format!("block-app-greetd-{}", uuid::Uuid::new_v4()));
    let environments = dir.join("environments");
    let link = Path::new("/usr/local/bin/block-app");
    assert!(
        !place_greetd(&environments, link).unwrap(),
        "without greetd there is nothing to list it in"
    );

    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&environments, "sway_hrdl\nsway").unwrap();
    assert!(place_greetd(&environments, link).unwrap());
    assert!(!place_greetd(&environments, link).unwrap());
    assert_eq!(
        std::fs::read_to_string(&environments).unwrap(),
        "sway_hrdl\nsway\nenv BEUI_PLATFORM=drm /usr/local/bin/block-app --desktop\n",
        "the other sessions stay and the app's is listed once"
    );

    let spaced = Path::new("/opt/my apps/bin/block-app");
    assert!(
        !place_greetd(&environments, spaced).unwrap(),
        "greeters split a line at spaces, so a path with one is not listed"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
