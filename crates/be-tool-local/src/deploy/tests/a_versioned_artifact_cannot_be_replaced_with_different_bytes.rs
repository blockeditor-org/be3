use super::*;
use std::{io::Write, net::TcpListener};

#[test]
fn a_versioned_artifact_cannot_be_replaced_with_different_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = Vec::new();
        let mut byte = [0];
        while !buffer.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            buffer.push(byte[0]);
        }
        assert!(buffer.starts_with(b"GET /data/hash/web/app.wasm HTTP/1.1"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nold")
            .unwrap();
    });
    let options = parse(
        ["--repository", ".", "--server", "server"]
            .map(str::to_owned)
            .into_iter(),
    )
    .unwrap();
    let agents = Agents::new();
    let store = Store {
        options: &options,
        agents: &agents,
        storage: &url,
        mirror: &url,
        key: "test key",
    };
    let error = store
        .put(
            "wasm",
            "data/hash/web/app.wasm",
            b"new",
            "application/wasm",
            true,
        )
        .unwrap_err();
    assert!(error.contains("refusing to overwrite immutable artifact"));
    server.join().unwrap();
}
