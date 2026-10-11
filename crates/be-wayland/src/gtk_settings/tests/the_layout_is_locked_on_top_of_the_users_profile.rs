use super::*;

#[test]
fn the_layout_is_locked_on_top_of_the_users_profile() {
    let dir = std::env::temp_dir().join(format!("be-wayland-gtk-test-{}", std::process::id()));
    let users = dir.with_extension("base");
    let base = users.join("profile");
    std::fs::create_dir_all(&users).unwrap();
    std::fs::write(&base, "user-db:user\nsystem-db:local\n").unwrap();
    let profile = write(&dir, Some(base.as_os_str())).unwrap();
    assert_eq!(
        std::fs::read_to_string(&profile).unwrap(),
        format!(
            "user-db:user\nsystem-db:local\nfile-db:{}\n",
            dir.join("gtk.db").display()
        )
    );
    let again = write(&dir, Some(profile.as_os_str())).unwrap();
    assert_eq!(
        std::fs::read_to_string(again)
            .unwrap()
            .matches(&format!("file-db:{}", dir.join("gtk.db").display()))
            .count(),
        1,
        "our own profile is never layered on itself"
    );

    let contents = database_contents().unwrap();
    let file = gvdb::read::File::from_bytes(contents.into()).unwrap();
    let table = file.hash_table().unwrap();
    let layout: String = table.get(BUTTON_LAYOUT).unwrap();
    assert_eq!(layout, LAYOUT);
    let locks = table.get_hash_table(".locks").unwrap();
    assert!(locks.keys().any(|key| key.unwrap() == BUTTON_LAYOUT));
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::remove_dir_all(&users).unwrap();
}
