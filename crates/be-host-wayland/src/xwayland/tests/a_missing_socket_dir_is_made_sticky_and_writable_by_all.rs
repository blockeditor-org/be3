use super::*;

#[test]
fn a_missing_socket_dir_is_made_sticky_and_writable_by_all() {
    let temp = std::env::temp_dir().join(format!("be-wayland-x11-dir-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("the test directory is made");
    let dir = temp.join(".X11-unix");

    prepare_socket_dir(&dir).expect("the missing directory is made");
    let mode = dir
        .metadata()
        .expect("the directory exists")
        .permissions()
        .mode();
    assert_eq!(mode & 0o7777, 0o1777);

    prepare_socket_dir(&dir).expect("an existing directory is kept");
    std::fs::remove_dir_all(&temp).expect("the test directory is removed");
}
