use super::*;

#[test]
fn xwayland_is_found_only_as_an_executable_on_path() {
    let temp = std::env::temp_dir().join(format!("be-wayland-x11-path-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let plain = temp.join("plain");
    let runnable = temp.join("runnable");
    std::fs::create_dir_all(&plain).expect("a directory is made");
    std::fs::create_dir_all(&runnable).expect("a directory is made");
    std::fs::write(plain.join(PROGRAM), b"").expect("a file is made");
    std::fs::write(runnable.join(PROGRAM), b"").expect("a file is made");
    std::fs::set_permissions(runnable.join(PROGRAM), std::fs::Permissions::from_mode(0o755))
        .expect("the file is made executable");

    let path = std::env::join_paths([&plain]).expect("a path");
    assert_eq!(find_program(PROGRAM, &path), None);
    let path = std::env::join_paths([&plain, &runnable]).expect("a path");
    assert_eq!(find_program(PROGRAM, &path), Some(runnable.join(PROGRAM)));
    std::fs::remove_dir_all(&temp).expect("the test directory is removed");
}
