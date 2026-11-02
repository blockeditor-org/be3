use super::*;

#[test]
fn a_socket_dir_that_is_a_file_is_named() {
    let temp = std::env::temp_dir().join(format!("be-wayland-x11-file-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("the test directory is made");
    let dir = temp.join(".X11-unix");
    std::fs::write(&dir, b"").expect("a file is made");

    let error = prepare_socket_dir(&dir).expect_err("a file is not a socket directory");
    assert!(matches!(error, StartError::SocketDirNotADirectory(_)));
    assert!(error.to_string().contains(".X11-unix, where X servers put their sockets, is not a directory"));
    std::fs::remove_dir_all(&temp).expect("the test directory is removed");
}
