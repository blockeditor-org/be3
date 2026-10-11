use super::*;

#[test]
fn the_layout_is_locked_on_top_of_the_users_profile() {
    let dir = std::env::temp_dir().join(format!("be-wayland-gtk-test-{}", std::process::id()));
    let base = dir.join("base");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&base, "user-db:user\nsystem-db:local\n").unwrap();
    let own = dir.join("dconf.profile");
    assert_eq!(
        base_profile(&own, Some(base.as_os_str())),
        "user-db:user\nsystem-db:local\n"
    );
    let profile = write(&dir).unwrap();
    assert!(base_profile(&own, Some(profile.as_os_str())).starts_with("user-db:user"));

    let contents = database_contents().unwrap();
    let file = gvdb::read::File::from_bytes(contents.into()).unwrap();
    let table = file.hash_table().unwrap();
    let layout: String = table.get(BUTTON_LAYOUT).unwrap();
    assert_eq!(layout, LAYOUT);
    let locks = table.get_hash_table(".locks").unwrap();
    assert!(locks.keys().any(|key| key.unwrap() == BUTTON_LAYOUT));
    std::fs::remove_dir_all(&dir).unwrap();
}
