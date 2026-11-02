use super::*;

#[test]
fn installing_replaces_the_line_an_earlier_install_listed_for_greetd() {
    let dir = std::env::temp_dir().join(format!("be-host-greetd-{}", uuid::Uuid::new_v4()));
    let environments = dir.join("environments");
    let link = Path::new("/usr/local/bin/be-host");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &environments,
        "sway\n/usr/local/bin/be-host --session\n/opt/bin/be-host --session\n",
    )
    .unwrap();
    assert!(place_greetd(&environments, link).unwrap());
    assert_eq!(
        std::fs::read_to_string(&environments).unwrap(),
        "sway\n/opt/bin/be-host --session\nenv BEUI_PLATFORM=drm /usr/local/bin/be-host --desktop\n",
        "the line that runs this path with --session goes, and another path's stays"
    );

    std::fs::write(
        &environments,
        "env BEUI_PLATFORM=drm /usr/local/bin/be-host --desktop\n/usr/local/bin/be-host --session",
    )
    .unwrap();
    assert!(
        !place_greetd(&environments, link).unwrap(),
        "the login session was already listed"
    );
    assert_eq!(
        std::fs::read_to_string(&environments).unwrap(),
        "env BEUI_PLATFORM=drm /usr/local/bin/be-host --desktop\n",
        "but the stale line still goes"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
