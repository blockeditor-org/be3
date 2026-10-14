use crate::Server;
use crate::runtime_dir::RuntimeDir;

#[test]
fn a_server_without_a_runtime_dir_listens_in_a_private_one_it_removes() {
    let temp = std::env::temp_dir().join(format!("be-wayland-private-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("the test directory is made");
    let runtime = RuntimeDir::private_in(&temp).expect("a private runtime directory is made");
    let server = Server::listening(runtime).expect("the server listens");
    let dir = server
        .runtime_dir()
        .expect("the server has a runtime directory")
        .to_owned();
    let name = server
        .socket_name()
        .expect("the server has a socket")
        .to_owned();
    assert_eq!(
        name, "wayland-0",
        "WAYLAND_DISPLAY is a name in XDG_RUNTIME_DIR, as every client library takes it"
    );
    std::os::unix::net::UnixStream::connect(dir.join(&name))
        .expect("a client reaches the socket through XDG_RUNTIME_DIR and WAYLAND_DISPLAY");

    drop(server);
    assert!(
        !dir.exists(),
        "the socket, its lock and the directory go with the server"
    );
    let _ = std::fs::remove_dir_all(&temp);
}
